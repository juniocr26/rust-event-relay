//! Concrete relay composition; errors exposed by main are sanitized static messages.
use reliable_event_relay::{
    application::publisher::PublishError,
    application::{
        self,
        worker::{self, PublisherLifecycle},
    },
    config::{Config, WorkerConfig},
    infrastructure::{
        postgres::PostgresOutboxRepository,
        rabbitmq::{RabbitMqConfig, RabbitMqPublisher},
    },
    telemetry,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::time::{Instant, timeout_at};

struct RabbitLifecycle(RabbitMqConfig);
impl PublisherLifecycle for RabbitLifecycle {
    type Publisher = RabbitMqPublisher;
    async fn connect(&self) -> Result<RabbitMqPublisher, PublishError> {
        RabbitMqPublisher::connect(self.0.clone()).await
    }
    fn usable(&self, publisher: &RabbitMqPublisher) -> bool {
        publisher.is_usable()
    }
    async fn close(&self, publisher: RabbitMqPublisher) -> Result<(), PublishError> {
        publisher.close().await
    }
}

use reliable_event_relay::application::observation::{Observer, Operation as Op, Outcome as Obs};
use reliable_event_relay::telemetry::metrics::{Exporter, ExporterConfig, Metrics, Process};
#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let app = Config::load().map_err(|_| "invalid application configuration (source redacted)")?;
    telemetry::init(&app.log_filter).map_err(|_| "invalid telemetry configuration")?;
    let config = WorkerConfig::from_lookup(|key| std::env::var(key).ok()).map_err(|e| {
        tracing::error!(error = %e, "invalid worker configuration");
        "invalid worker configuration"
    })?;
    let metrics = Metrics::new(Process::Relay);
    let exporter_config =
        ExporterConfig::from_lookup(|key| std::env::var(key).ok(), app.http_addr)?;
    let exporter = Exporter::start(exporter_config.address(Process::Relay), metrics.clone())
        .await
        .map_err(|_| "metrics bind failed")?;
    metrics.configure(config.pool_max, 0, false);
    let rabbit = RabbitMqConfig::from_lookup(|key| std::env::var(key).ok())
        .map_err(|_| "invalid RabbitMQ configuration (source redacted)")?;
    let required = |key| {
        std::env::var(key)
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or("missing PostgreSQL configuration")
    };
    let port = std::env::var("POSTGRES_PORT")
        .unwrap_or_else(|_| "5432".into())
        .parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
        .ok_or("invalid PostgreSQL port")?;
    let options = PgConnectOptions::new()
        .host(&std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres".into()))
        .port(port)
        .username(&required("POSTGRES_USER")?)
        .password(&required("POSTGRES_PASSWORD")?)
        .database(&required("POSTGRES_DB")?)
        .options([
            ("statement_timeout", config.statement_ms.to_string()),
            ("lock_timeout", config.lock_ms.to_string()),
        ]);
    let shutdown = application::shutdown_signal();
    tokio::pin!(shutdown);
    let pool = tokio::select! { biased; () = &mut shutdown => return Ok(()),
    result = PgPoolOptions::new().max_connections(config.pool_max).acquire_timeout(config.pool_acquire).connect_with(options) => result.map_err(|_| "PostgreSQL setup unavailable (source redacted)")? };
    // Read-only validation of the columns used by ownership operations; never migrates.
    let schema = tokio::select! { biased; () = &mut shutdown => None,
    result = sqlx::query("SELECT id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at, correlation_id, causation_id, payload, status, attempt_count, available_at, created_at, processed_at, ownership_token, acquired_at, expires_at, retry_cycle, cycle_baseline, cycle_maximum, state_revision, diagnostic_code, diagnostic_at, failure_evidence, terminal_at, terminal_reason FROM relay.outbox_events LIMIT 0").execute(&pool) => Some(result) };
    let setup_failed = schema.as_ref().is_some_and(Result::is_err);
    let deadline = if !matches!(schema, Some(Ok(_))) {
        Instant::now() + config.shutdown
    } else {
        tracing::info!(config = ?config, "relay started; sequential admission");
        let repository = PostgresOutboxRepository::new(pool.clone());
        worker::run_observed(
            &repository,
            &RabbitLifecycle(rabbit),
            &config,
            &mut shutdown,
            metrics.as_ref(),
        )
        .await
    };
    let cleanup_expired = timeout_at(deadline, pool.close()).await.is_err();
    if cleanup_expired {
        tracing::warn!("pool cleanup exhausted shutdown budget");
    }
    if setup_failed {
        return Err(
            "required ownership schema unavailable; inspect migrations 20261009000000/20261012000000 and database privileges (source redacted)",
        );
    }
    metrics.event(
        Op::Shutdown,
        if cleanup_expired {
            Obs::Deadline
        } else {
            Obs::Completed
        },
    );
    exporter.shutdown().await;
    tracing::info!("relay stopped");
    Ok(())
}
