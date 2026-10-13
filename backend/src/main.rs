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

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let config = Config::load().map_err(|_| "invalid application configuration")?;
    telemetry::init(&config.log_filter).map_err(|_| "invalid telemetry configuration")?;
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
        router = router.merge(operator_http::router(
            PostgresOutboxRepository::new(connected.clone()),
            credentials,
        ));
        pool = Some(connected);
    }
    let listener = tokio::net::TcpListener::bind(config.http_addr)
        .await
        .map_err(|_| "HTTP bind failed")?;
    tracing::info!(environment = %config.environment, address = %listener.local_addr().map_err(|_| "HTTP address unavailable")?, "application started");
    axum::serve(listener, router)
        .with_graceful_shutdown(application::shutdown_signal())
        .await
        .map_err(|_| "HTTP server failed")?;
    if let Some(pool) = pool {
        pool.close().await;
    }
    tracing::info!("application stopped");
    Ok(())
}
