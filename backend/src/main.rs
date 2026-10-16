use reliable_event_relay::{
    application,
    config::Config,
    infrastructure::{
        operator_http::{self, OperatorCredentials},
        postgres::PostgresOutboxRepository,
    },
    telemetry,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::time::Duration;

use reliable_event_relay::application::observation::{Observer, Operation as Op, Outcome as Obs};
use reliable_event_relay::telemetry::metrics::{Exporter, ExporterConfig, Metrics, Process};
#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let config = Config::load().map_err(|_| "invalid application configuration")?;
    telemetry::init(&config.log_filter).map_err(|_| "invalid telemetry configuration")?;
    let metrics = Metrics::new(Process::Http);
    let exporter_config =
        ExporterConfig::from_lookup(|key| std::env::var(key).ok(), config.http_addr)?;
    let exporter = Exporter::start(exporter_config.address(Process::Http), metrics.clone())
        .await
        .map_err(|_| "metrics bind failed")?;
    let mut router = application::router();
    let mut pool = None;
    // Opt-in only. No connection or migration is required for liveness-only HTTP.
    if let Some(raw) = std::env::var("REPLAY_OPERATORS")
        .ok()
        .filter(|s| !s.is_empty())
    {
        let credentials = OperatorCredentials::parse(&raw)?;
        let required = |key| {
            std::env::var(key)
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or("missing replay database configuration")
        };
        let options = PgConnectOptions::new()
            .host(&std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres".into()))
            .port(
                std::env::var("POSTGRES_PORT")
                    .unwrap_or_else(|_| "5432".into())
                    .parse()
                    .map_err(|_| "invalid database port")?,
            )
            .username(&required("POSTGRES_USER")?)
            .password(&required("POSTGRES_PASSWORD")?)
            .database(&required("POSTGRES_DB")?)
            .options([("statement_timeout", "5000"), ("lock_timeout", "3000")]);
        let connected = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(|_| "replay database unavailable (source redacted)")?;
        sqlx::query("SELECT request_id,actor,scope,event_id,intent,receipt,prior_terminal,recorded_at FROM relay.replay_requests LIMIT 0").execute(&connected).await.map_err(|_| "replay migration/privileges required (source redacted)")?;
        metrics.configure(2, 0, true);
        router = router.merge(operator_http::router_observed(
            PostgresOutboxRepository::new(connected.clone()),
            credentials,
            metrics.clone(),
        ));
        pool = Some(connected);
    }
    let http_metrics = metrics.clone();
    router = router.layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let metrics = http_metrics.clone();
            async move {
                reliable_event_relay::application::observation::observe(
                    metrics.as_ref(),
                    Op::Http,
                    next.run(request),
                    |response| {
                        if response.status().is_success() {
                            Obs::Completed
                        } else {
                            Obs::Rejected
                        }
                    },
                )
                .await
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind(config.http_addr)
        .await
        .map_err(|_| "HTTP bind failed")?;
    tracing::info!(environment = %config.environment, address = %listener.local_addr().map_err(|_| "HTTP address unavailable")?, "application started");
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, router).with_graceful_shutdown(async {
        let _ = stop_rx.await;
    });
    // Convert IntoFuture explicitly; bound connection drain after the signal.
    let server = std::future::IntoFuture::into_future(server);
    tokio::pin!(server);
    let mut cleanup_expired = false;
    tokio::select! {
        result=&mut server=>result.map_err(|_| "HTTP server failed")?,
        ()=application::shutdown_signal()=>{
            let _=stop_tx.send(());
            if tokio::time::timeout(Duration::from_secs(5), &mut server).await.is_err() { cleanup_expired=true; }
        }
    }
    if let Some(pool) = pool {
        cleanup_expired |= tokio::time::timeout(Duration::from_secs(5), pool.close())
            .await
            .is_err();
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
    tracing::info!("application stopped");
    Ok(())
}
