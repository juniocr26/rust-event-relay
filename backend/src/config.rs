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

/// Operational durations are milliseconds; lease conversion is checked.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub lease: crate::domain::delivery::LeaseDuration,
    /// Independently observed exact ID/cycle/revision; at most 32 per process run.
    maintenance: Vec<crate::persistence::ExhaustedEvent>,
    pub idle: std::time::Duration,
    pub failure: std::time::Duration,
    pub reconnect: std::time::Duration,
    pub pool_max: u32,
    pub pool_acquire: std::time::Duration,
    pub statement_ms: u64,
    pub lock_ms: u64,
    pub drain: std::time::Duration,
    pub shutdown: std::time::Duration,
}
impl WorkerConfig {
    pub fn maintenance(&self) -> &[crate::persistence::ExhaustedEvent] {
        &self.maintenance
    }
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let value = |key: &str, default: u64, max: u64| -> Result<u64, ConfigError> {
            let n = get(key)
                .map_or(Ok(default), |s| s.parse::<u64>())
                .map_err(|_| ConfigError(format!("{key} must be a positive integer")))?;
            if n == 0 || n > max {
                return Err(ConfigError(format!("{key} must be in 1..={max}")));
            }
            Ok(n)
        };
        let duration =
            |key, default, max| value(key, default, max).map(std::time::Duration::from_millis);
        let lease_ms = value("RELAY_LEASE_MS", 60000, 3600000)?;
        let micros = lease_ms
            .checked_mul(1000)
            .and_then(|n| i64::try_from(n).ok())
            .ok_or_else(|| ConfigError("RELAY_LEASE_MS conversion overflow".into()))?;
        let raw = get("RELAY_MAINTENANCE_CANDIDATES").unwrap_or_default();
        if raw.len() > 4096 {
            return Err(ConfigError("maintenance input exceeds 4096 bytes".into()));
        }
        let mut maintenance = Vec::new();
        if !raw.is_empty() {
            for item in raw.split(',') {
                if maintenance.len() == 32 {
                    return Err(ConfigError("at most 32 maintenance candidates".into()));
                }
                let fields: Vec<_> = item.split(':').collect();
                let invalid = || {
                    ConfigError("maintenance requires UUID:cycle:revision with unique IDs".into())
                };
                if fields.len() != 3 {
                    return Err(invalid());
                }
                let id = fields[0].parse::<uuid::Uuid>().map_err(|_| invalid())?;
                let cycle = fields[1].parse::<i32>().map_err(|_| invalid())?;
                let revision = fields[2].parse::<i64>().map_err(|_| invalid())?;
                if maintenance
                    .iter()
                    .any(|c: &crate::persistence::ExhaustedEvent| c.id() == id)
                {
                    return Err(invalid());
                }
                maintenance.push(
                    crate::persistence::ExhaustedEvent::new(id, cycle, revision)
                        .map_err(|_| invalid())?,
                );
            }
        }
        let config = Self {
            maintenance,
            lease: crate::domain::delivery::LeaseDuration::from_micros(micros)
                .map_err(|_| ConfigError("invalid lease".into()))?,
            idle: duration("RELAY_IDLE_MS", 1000, 60000)?,
            failure: duration("RELAY_FAILURE_MS", 1000, 60000)?,
            reconnect: duration("RELAY_RECONNECT_MS", 5000, 60000)?,
            pool_max: u32::try_from(value("RELAY_POOL_MAX", 2, 16)?).expect("bounded pool limit"),
            pool_acquire: duration("RELAY_POOL_ACQUIRE_MS", 5000, 60000)?,
            statement_ms: value("RELAY_STATEMENT_MS", 5000, 60000)?,
            lock_ms: value("RELAY_LOCK_MS", 3000, 60000)?,
            drain: duration("RELAY_DRAIN_MS", 15000, 20000)?,
            shutdown: duration("RELAY_SHUTDOWN_MS", 25000, 25000)?,
        };
        if config.drain >= config.shutdown {
            return Err(ConfigError(
                "RELAY_DRAIN_MS must be less than RELAY_SHUTDOWN_MS".into(),
            ));
        }
        Ok(config)
    }
}

/// Validated sequential consumer limits; milliseconds, no unlimited prefetch.
/// Limites validados do consumidor sequencial; milissegundos, sem prefetch ilimitado.
#[derive(Debug, Clone)]
pub struct ConsumerRuntimeConfig {
    pub name: crate::domain::consumer::ConsumerName,
    pub prefetch: u16,
    pub operation: std::time::Duration,
    pub reconnect: std::time::Duration,
    pub drain: std::time::Duration,
    pub shutdown: std::time::Duration,
    pub pool_max: u32,
    pub pool_acquire: std::time::Duration,
    pub statement_ms: u64,
    pub lock_ms: u64,
}
impl ConsumerRuntimeConfig {
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let number = |key: &str, default, max| {
            let n = get(key)
                .map_or(Ok(default), |v| v.parse::<u64>())
                .map_err(|_| ConfigError(format!("invalid {key}")))?;
            if n == 0 || n > max {
                Err(ConfigError(format!("{key} must be in 1..={max}")))
            } else {
                Ok(n)
            }
        };
        let duration =
            |key, default, max| number(key, default, max).map(std::time::Duration::from_millis);
        let result = Self {
            name: crate::domain::consumer::ConsumerName::new(
                get("CONSUMER_NAME").unwrap_or_else(|| "inventory.stock.v1".into()),
            )
            .map_err(|_| ConfigError("invalid CONSUMER_NAME".into()))?,
            prefetch: u16::try_from(number("CONSUMER_PREFETCH", 1, 32)?)
                .expect("validated prefetch"),
            operation: duration("CONSUMER_OPERATION_MS", 10000, 60000)?,
            reconnect: duration("CONSUMER_RECONNECT_MS", 5000, 60000)?,
            drain: duration("CONSUMER_DRAIN_MS", 15000, 20000)?,
            shutdown: duration("CONSUMER_SHUTDOWN_MS", 25000, 25000)?,
            pool_max: u32::try_from(number("CONSUMER_POOL_MAX", 2, 16)?).expect("validated pool"),
            pool_acquire: duration("CONSUMER_POOL_ACQUIRE_MS", 5000, 60000)?,
            statement_ms: number("CONSUMER_STATEMENT_MS", 5000, 60000)?,
            lock_ms: number("CONSUMER_LOCK_MS", 3000, 60000)?,
        };
        if result.drain >= result.shutdown {
            return Err(ConfigError(
                "consumer drain must be less than shutdown".into(),
            ));
        }
        Ok(result)
    }
}
