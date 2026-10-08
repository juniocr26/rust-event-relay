//! Owned connection/channel, bounded waits, no automatic recovery or publication retries.
use crate::{
    application::publisher::{EventPublisher, PublishError, PublishErrorKind as Kind},
    domain::event::EventEnvelope,
};
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, ExchangeKind,
    options::*, types::FieldTable,
};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{sync::Mutex, time::timeout};

/// Infrastructure-only topology. Resource names must start with `relay.`.
#[derive(Clone, Debug)]
pub struct Topology {
    pub exchange: String,
    pub queue: String,
    pub routing_key: String,
}
impl Default for Topology {
    fn default() -> Self {
        Self {
            exchange: "relay.events".into(),
            queue: "relay.local".into(),
            routing_key: "event".into(),
        }
    }
}

/// Validated connection and operation deadlines. Credentials are never formatted.
pub struct RabbitMqConfig {
    uri: String,
    topology: Topology,
    connect_timeout: Duration,
    publish_timeout: Duration,
}
impl fmt::Debug for RabbitMqConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RabbitMqConfig { credentials: [REDACTED] }")
    }
}
impl RabbitMqConfig {
    pub fn new(
        uri: String,
        topology: Topology,
        connect_timeout: Duration,
        publish_timeout: Duration,
    ) -> Result<Self, PublishError> {
        let parsed =
            url::Url::parse(&uri).map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
        if parsed.scheme() != "amqp"
            || parsed.host_str().is_none()
            || parsed.username().is_empty()
            || parsed.password().is_none_or(str::is_empty)
            || !topology.exchange.starts_with("relay.")
            || !topology.queue.starts_with("relay.")
            || topology.routing_key.trim().is_empty()
            || [&topology.exchange, &topology.queue, &topology.routing_key]
                .iter()
                .any(|s| s.len() > 255)
            || [connect_timeout, publish_timeout]
                .iter()
                .any(|d| d.is_zero() || *d > Duration::from_secs(300))
        {
            return Err(PublishError::new(Kind::Unavailable));
        }
        Ok(Self {
            uri,
            topology,
            connect_timeout,
            publish_timeout,
        })
    }

    /// Builds an escaped AMQP URI from separate environment fields, without logging secrets.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, PublishError> {
        let required = |key| {
            get(key)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| PublishError::new(Kind::Unavailable))
        };
        let mut uri = url::Url::parse("amqp://rabbitmq:5672/relay").expect("static URL");
        uri.set_username(&required("RABBITMQ_DEFAULT_USER")?)
            .map_err(|_| PublishError::new(Kind::Unavailable))?;
        uri.set_password(Some(&required("RABBITMQ_DEFAULT_PASS")?))
            .map_err(|_| PublishError::new(Kind::Unavailable))?;
        uri.set_host(Some(
            &get("RABBITMQ_HOST").unwrap_or_else(|| "rabbitmq".into()),
        ))
        .map_err(|_| PublishError::new(Kind::Unavailable))?;
        let port = get("RABBITMQ_PORT")
            .unwrap_or_else(|| "5672".into())
            .parse::<u16>()
            .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
        if port == 0 {
            return Err(PublishError::new(Kind::Unavailable));
        }
        uri.set_port(Some(port))
            .map_err(|_| PublishError::new(Kind::Unavailable))?;
        // Vhost is one percent-encoded path segment, including embedded slashes.
        uri.path_segments_mut()
            .map_err(|_| PublishError::new(Kind::Unavailable))?
            .clear()
            .push(&get("RABBITMQ_DEFAULT_VHOST").unwrap_or_else(|| "relay".into()));
        let deadline = |key| -> Result<Duration, PublishError> {
            let ms = get(key)
                .unwrap_or_else(|| "10000".into())
                .parse::<u64>()
                .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
            Ok(Duration::from_millis(ms))
        };
        let topology = Topology {
            exchange: get("RABBITMQ_EXCHANGE").unwrap_or_else(|| "relay.events".into()),
            queue: get("RABBITMQ_QUEUE").unwrap_or_else(|| "relay.local".into()),
            routing_key: get("RABBITMQ_ROUTING_KEY").unwrap_or_else(|| "event".into()),
        };
        Self::new(
            uri.into(),
            topology,
            deadline("RABBITMQ_CONNECT_TIMEOUT_MS")?,
            deadline("RABBITMQ_PUBLISH_TIMEOUT_MS")?,
        )
    }
}

/// Reuses one connection and channel; serializes publication to bound in-flight work to one.
/// On timeout/error/cancellation after sending, the channel is retired; reconnect explicitly.
pub struct RabbitMqPublisher {
    connection: Connection,
    channel: Channel,
    config: RabbitMqConfig,
    gate: Mutex<()>,
    retired: AtomicBool,
}
impl RabbitMqPublisher {
    pub async fn connect(config: RabbitMqConfig) -> Result<Self, PublishError> {
        let connection = timeout(
            config.connect_timeout,
            Connection::connect(&config.uri, ConnectionProperties::default()),
        )
        .await
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
        let setup = timeout(config.connect_timeout, async {
            let channel = connection.create_channel().await?;
            channel
                .exchange_declare(
                    config.topology.exchange.clone().into(),
                    ExchangeKind::Direct,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            channel
                .queue_declare(
                    config.topology.queue.clone().into(),
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            channel
                .queue_bind(
                    config.topology.queue.clone().into(),
                    config.topology.exchange.clone().into(),
                    config.topology.routing_key.clone().into(),
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            channel
                .confirm_select(ConfirmSelectOptions::default())
                .await?;
            Ok::<_, lapin::Error>(channel)
        })
        .await;
        let channel = match setup {
            Ok(Ok(channel)) => channel,
            result => {
                let _ = timeout(
                    config.connect_timeout,
                    connection.close(200, "setup failed".into()),
                )
                .await;
                return Err(match result {
                    Ok(Err(e)) => PublishError::with_source(Kind::Unavailable, e),
                    Err(e) => PublishError::with_source(Kind::Unavailable, e),
                    Ok(Ok(_)) => unreachable!(),
                });
            }
        };
        Ok(Self {
            connection,
            channel,
            config,
            gate: Mutex::new(()),
            retired: AtomicBool::new(false),
        })
    }

    /// Call after awaiting in-flight publications; consumes the adapter and bounds graceful close.
    pub async fn close(self) -> Result<(), PublishError> {
        timeout(
            self.config.connect_timeout,
            self.connection.close(200, "publisher shutdown".into()),
        )
        .await
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))
    }
}
fn classify(confirmation: Confirmation) -> Result<(), PublishError> {
    match confirmation {
        Confirmation::Ack(None) => Ok(()),
        Confirmation::Ack(Some(_)) => Err(PublishError::new(Kind::Unroutable)),
        Confirmation::Nack(_) => Err(PublishError::new(Kind::Rejected)),
        Confirmation::NotRequested => Err(PublishError::new(Kind::Uncertain)),
    }
}
impl EventPublisher for RabbitMqPublisher {
    async fn publish(&self, event: &EventEnvelope) -> Result<(), PublishError> {
        let body = serde_json::to_vec(event)
            .map_err(|e| PublishError::with_source(Kind::Serialization, e))?;
        let _gate = timeout(self.config.publish_timeout, self.gate.lock())
            .await
            .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
        if self.retired.load(Ordering::Acquire)
            || !self.channel.status().connected()
            || !self.connection.status().connected()
        {
            return Err(PublishError::new(Kind::Unavailable));
        }
        // Remains set if this future is dropped or an operation has an uncertain outcome.
        self.retired.store(true, Ordering::Release);
        let result = timeout(self.config.publish_timeout, async {
            self.channel
                .basic_publish(
                    self.config.topology.exchange.clone().into(),
                    self.config.topology.routing_key.clone().into(),
                    BasicPublishOptions {
                        mandatory: true,
                        ..Default::default()
                    },
                    &body,
                    BasicProperties::default()
                        .with_content_type("application/json".into())
                        .with_delivery_mode(2)
                        .with_message_id(event.id().to_string().into()),
                )
                .await?
                .await
        })
        .await;
        let confirmation = result
            .map_err(|e| PublishError::with_source(Kind::Uncertain, e))?
            .map_err(|e| PublishError::with_source(Kind::Uncertain, e))?;
        let result = classify(confirmation);
        if result
            .as_ref()
            .err()
            .is_none_or(|e| e.kind() != Kind::Uncertain)
        {
            self.retired.store(false, Ordering::Release);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    #[test]
    fn confirms_and_sources_are_sanitized() {
        assert!(classify(Confirmation::Ack(None)).is_ok());
        assert_eq!(
            classify(Confirmation::Nack(None)).unwrap_err().kind(),
            Kind::Rejected
        );
        assert_eq!(
            classify(Confirmation::NotRequested).unwrap_err().kind(),
            Kind::Uncertain
        );
        let e =
            PublishError::with_source(Kind::Unavailable, std::io::Error::other("secret password"));
        assert!(!format!("{e:?} {e}").contains("secret"));
        assert!(
            e.source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .is_some()
        );
    }
    #[tokio::test]
    async fn stalled_handshake_is_bounded_and_retains_timeout_source() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = RabbitMqConfig::new(
            format!("amqp://user:secret@{address}/relay"),
            Topology::default(),
            Duration::from_millis(40),
            Duration::from_secs(1),
        )
        .unwrap();
        let connect = RabbitMqPublisher::connect(config);
        let error = tokio::time::timeout(Duration::from_secs(2), async {
            let (connection, accepted) = tokio::join!(connect, listener.accept());
            let _held_socket = accepted.unwrap();
            connection.err().expect("handshake must time out")
        })
        .await
        .unwrap();
        assert_eq!(error.kind(), Kind::Unavailable);
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<tokio::time::error::Elapsed>()
                .is_some()
        );
    }

    #[tokio::test]
    async fn refused_connection_retains_driver_source() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let config = RabbitMqConfig::new(
            format!("amqp://user:secret@{address}/relay"),
            Topology::default(),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap();
        let error = RabbitMqPublisher::connect(config)
            .await
            .err()
            .expect("refused connection");
        assert_eq!(error.kind(), Kind::Unavailable);
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<lapin::Error>()
                .is_some()
        );
    }

    #[tokio::test]
    #[ignore = "requires real RabbitMQ and explicit credentials"]
    async fn closed_owned_connection_is_unavailable() {
        let mut config = RabbitMqConfig::from_lookup(|key| std::env::var(key).ok()).unwrap();
        let id = uuid::Uuid::now_v7().simple().to_string();
        config.topology.exchange = format!("relay.it.{id}.exchange");
        config.topology.queue = format!("relay.it.{id}.queue");
        let publisher = RabbitMqPublisher::connect(config).await.unwrap();
        publisher
            .channel
            .queue_delete(
                publisher.config.topology.queue.clone().into(),
                QueueDeleteOptions::default(),
            )
            .await
            .unwrap();
        publisher
            .channel
            .exchange_delete(
                publisher.config.topology.exchange.clone().into(),
                ExchangeDeleteOptions::default(),
            )
            .await
            .unwrap();
        publisher
            .connection
            .close(200, "test closed connection".into())
            .await
            .unwrap();
        let event = EventEnvelope::new("test", "test", "1", 1, serde_json::Value::Null).unwrap();
        assert_eq!(
            publisher.publish(&event).await.unwrap_err().kind(),
            Kind::Unavailable
        );
    }

    #[test]
    fn validates_deadlines_and_redacts_config() {
        for d in [Duration::ZERO, Duration::from_secs(301)] {
            assert!(
                RabbitMqConfig::new(
                    "amqp://user:secret@rabbitmq/relay".into(),
                    Topology::default(),
                    d,
                    Duration::from_secs(1)
                )
                .is_err()
            );
        }
        let c = RabbitMqConfig::from_lookup(|key| match key {
            "RABBITMQ_DEFAULT_USER" => Some("user@name".into()),
            "RABBITMQ_DEFAULT_PASS" => Some("secret:/@".into()),
            "RABBITMQ_DEFAULT_VHOST" => Some("a/b".into()),
            _ => None,
        })
        .unwrap();
        assert!(c.uri.contains("a%2Fb"));
        assert!(!format!("{c:?}").contains("secret"));
    }
}
