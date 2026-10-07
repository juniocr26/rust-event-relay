use tracing_subscriber::EnvFilter;

pub fn init(filter: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = EnvFilter::try_new(filter)?;
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init()?;
    Ok(())
}
