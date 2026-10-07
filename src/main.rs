use reliable_event_relay::{application, config::Config, telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    telemetry::init(&config.log_filter)?;
    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(environment = %config.environment, address = %listener.local_addr()?, "application started");
    application::serve(listener, application::shutdown_signal()).await?;
    tracing::info!("application stopped");
    Ok(())
}
