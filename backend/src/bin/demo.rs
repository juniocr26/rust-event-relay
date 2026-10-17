//! Dedicated demo HTTP composition, never enabled in shared HTTP.
//! Composição HTTP exclusiva da demonstração, sem ativação no HTTP compartilhado.
use reliable_event_relay::{
    application,
    infrastructure::{demo_http, postgres::demo::DemoRepository},
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::time::Duration;
#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let required = |k| std::env::var(k).map_err(|_| "missing demo configuration");
    if required("POSTGRES_DB")? != "relay_demo61" || required("POSTGRES_HOST")? != "postgres" {
        return Err("dedicated demo database fingerprint required");
    }
    let options = PgConnectOptions::new()
        .host("postgres")
        .port(5432)
        .database("relay_demo61")
        .username(&required("POSTGRES_USER")?)
        .password(&required("POSTGRES_PASSWORD")?)
        .options([("statement_timeout", "3000"), ("lock_timeout", "1000")]);
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with(options)
        .await
        .map_err(|_| "demo database unavailable")?;
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .map_err(|_| "demo fingerprint unavailable")?;
    if name != "relay_demo61" {
        return Err("demo fingerprint mismatch");
    }
    let router = demo_http::router(
        DemoRepository::new(pool.clone()),
        required("DEMO_TOKEN")?,
        required("DEMO_ORIGIN")?,
    )?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .map_err(|_| "demo bind failed")?;
    axum::serve(listener, router)
        .with_graceful_shutdown(application::shutdown_signal())
        .await
        .map_err(|_| "demo HTTP failed")?;
    pool.close().await;
    Ok(())
}
