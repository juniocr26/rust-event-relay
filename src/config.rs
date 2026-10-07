use std::{error::Error, fmt, net::SocketAddr};

#[derive(Debug)]
pub struct Config {
    pub environment: String,
    pub http_addr: SocketAddr,
    pub log_filter: String,
}

#[derive(Debug)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ConfigError {}

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self::from_lookup(|key| std::env::var(key).ok())?)
    }

    // Explicit lookup makes parsing testable without mutating process environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let environment = get("APP_ENV").unwrap_or_else(|| "development".into());
        if environment.trim().is_empty() {
            return Err(ConfigError("APP_ENV must not be empty".into()));
        }
        let address = get("HTTP_ADDR").unwrap_or_else(|| "0.0.0.0:8080".into());
        let http_addr = address
            .parse()
            .map_err(|_| ConfigError("HTTP_ADDR must be an IP socket address".into()))?;
        let log_filter = get("RUST_LOG").unwrap_or_else(|| "info".into());
        Ok(Self {
            environment,
            http_addr,
            log_filter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_valid() {
        let config = Config::from_lookup(|_| None).unwrap();
        assert_eq!(config.http_addr, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.environment, "development");
    }
    #[test]
    fn rejects_invalid_address_and_empty_environment() {
        assert!(
            Config::from_lookup(|key| (key == "HTTP_ADDR").then(|| "localhost:invalid".into()))
                .is_err()
        );
        assert!(Config::from_lookup(|key| (key == "APP_ENV").then(String::new)).is_err());
    }
}
