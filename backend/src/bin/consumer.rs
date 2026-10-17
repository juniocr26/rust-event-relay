//! Dedicated consumer composition; never runs migrations or changes credentials.
//! Composição dedicada; nunca executa migrations ou altera credenciais.
use reliable_event_relay::application::observation::{Observer, Operation as Op, Outcome as Obs};
use reliable_event_relay::telemetry::metrics::{Exporter, ExporterConfig, Metrics, Process};
use reliable_event_relay::telemetry::readiness::DatabaseObservation;
use reliable_event_relay::{
    application,
    config::{Config, ConsumerRuntimeConfig},
    infrastructure::{
        postgres::PostgresInventoryProcessor,
        rabbitmq::{RabbitConsumerConfig, run_consumer_observed},
    },
    telemetry,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::time::{Instant, timeout_at};
#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let app = Config::load().map_err(|_| "invalid application configuration (source redacted)")?;
    telemetry::init(&app.log_filter).map_err(|_| "invalid telemetry configuration")?;
    let config = ConsumerRuntimeConfig::from_lookup(|key| std::env::var(key).ok())
        .map_err(|_| "invalid consumer configuration")?;
    let metrics = Metrics::new(Process::Consumer);
    let exporter_config =
        ExporterConfig::from_lookup(|key| std::env::var(key).ok(), app.http_addr)?;
    let exporter = Exporter::start(exporter_config.address(Process::Consumer), metrics.clone())
        .await
        .map_err(|_| "metrics bind failed")?;
    metrics.configure(config.pool_max, config.prefetch, false);
    let rabbit = RabbitConsumerConfig::from_lookup(|key| std::env::var(key).ok(), config.prefetch)
        .map_err(|_| "invalid source/quarantine configuration")?;
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
    let mut shutdown = std::pin::pin!(async {
        application::shutdown_signal().await;
        metrics.readiness.stop();
    });
    let pool = tokio::select! {biased; ()=&mut shutdown=>return Ok(()),result=PgPoolOptions::new().max_connections(config.pool_max).acquire_timeout(config.pool_acquire).connect_with(options.clone())=>result.map_err(|_|"consumer PostgreSQL unavailable (source redacted)")?};
    let schema = tokio::select! {biased; ()=&mut shutdown=>None,result=async {
        for query in reliable_event_relay::telemetry::readiness::CONSUMER_SCHEMA {
            sqlx::query(query).execute(&pool).await?;
        }
        Ok::<(), sqlx::Error>(())
    }=>Some(result)};
    let observation = DatabaseObservation::start(
        options,
        reliable_event_relay::telemetry::readiness::CONSUMER_SCHEMA,
        metrics.readiness.clone(),
    );
    let failed = schema.as_ref().is_some_and(Result::is_err);
    let deadline = if matches!(schema, Some(Ok(_))) {
        metrics.readiness.admit();
        tracing::info!(
            prefetch = config.prefetch,
            "consumer starting; sequential admission; sources redacted"
        );
        let processor = PostgresInventoryProcessor::new(pool.clone());
        let processor = if std::env::var("DEMO_ENABLED").as_deref() == Ok("true") {
            processor.with_demo_evidence()
        } else {
            processor
        };
        run_consumer_observed(
            &processor,
            &config,
            &rabbit,
            &mut shutdown,
            metrics.as_ref(),
        )
        .await
    } else {
        Instant::now() + config.shutdown
    };
    metrics.readiness.stop();
    observation.shutdown().await;
    let cleanup_expired = timeout_at(deadline, pool.close()).await.is_err();
    if cleanup_expired {
        tracing::warn!("consumer pool cleanup deadline exhausted");
    }
    if failed {
        return Err(
            "consumer schema unavailable; apply migration 20261014000000 and verify privileges before rollout (source redacted)",
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
    tracing::info!("consumer stopped; unacknowledged deliveries remain recoverable");
    Ok(())
}
