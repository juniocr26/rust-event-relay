//! Explicit opt-in broker tests. Every case owns only UUID-prefixed relay.* resources.
use lapin::{Connection, ConnectionProperties, options::*, types::FieldTable};
use reliable_event_relay::{
    application::publisher::{EventPublisher, PublishErrorKind},
    domain::event::EventEnvelope,
    infrastructure::rabbitmq::{RabbitMqConfig, RabbitMqPublisher, Topology},
};
use std::time::Duration;

fn config(topology: Topology) -> RabbitMqConfig {
    RabbitMqConfig::new(
        uri(),
        topology,
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .unwrap()
}
fn uri() -> String {
    // Use the same validated escaped environment configuration for test-only consumers.
    let mut uri = url::Url::parse("amqp://rabbitmq/relay").unwrap();
    uri.set_username(&std::env::var("RABBITMQ_DEFAULT_USER").unwrap())
        .unwrap();
    uri.set_password(Some(&std::env::var("RABBITMQ_DEFAULT_PASS").unwrap()))
        .unwrap();
    uri.set_host(Some(
        &std::env::var("RABBITMQ_HOST").unwrap_or_else(|_| "rabbitmq".into()),
    ))
    .unwrap();
    uri.set_port(Some(
        std::env::var("RABBITMQ_PORT")
            .unwrap_or_else(|_| "5672".into())
            .parse()
            .unwrap(),
    ))
    .unwrap();
    uri.path_segments_mut()
        .unwrap()
        .clear()
        .push(&std::env::var("RABBITMQ_DEFAULT_VHOST").unwrap_or_else(|_| "relay".into()));
    uri.into()
}

#[tokio::test]
#[ignore = "requires a real RabbitMQ broker and explicit credentials"]
async fn routed_envelope_properties_and_mandatory_return() {
    let id = uuid::Uuid::now_v7().simple().to_string();
    let topology = Topology {
        exchange: format!("relay.it.{id}.exchange"),
        queue: format!("relay.it.{id}.queue"),
        routing_key: "test".into(),
    };
    let connection = tokio::time::timeout(
        Duration::from_secs(10),
        Connection::connect(&uri(), ConnectionProperties::default()),
    )
    .await
    .unwrap()
    .unwrap();
    let channel = connection.create_channel().await.unwrap();
    let owned = topology.clone();
    let case_channel = channel.clone();
    // Catch panic/task errors and still perform scoped cleanup outside the test task.
    let mut case = tokio::spawn(async move {
        let publisher = RabbitMqPublisher::connect(config(owned.clone()))
            .await
            .unwrap();
        let payload = serde_json::from_str(r#"{"precise":123456789012345678901234567890.123456789,"nested":[null,true,"á"],"exponent":1.23456789e+99}"#).unwrap();
        let event = EventEnvelope::new("test.created", "test", "42", 1, payload)
            .unwrap()
            .with_correlation_id(uuid::Uuid::now_v7())
            .with_causation_id(uuid::Uuid::now_v7());
        publisher.publish(&event).await.unwrap();
        let received = case_channel
            .basic_get(owned.queue.clone().into(), BasicGetOptions::default())
            .await
            .unwrap()
            .expect("confirmed message");
        assert_eq!(
            serde_json::from_slice::<EventEnvelope>(&received.data).unwrap(),
            event
        );
        assert_eq!(
            received
                .properties
                .content_type()
                .as_ref()
                .unwrap()
                .as_str(),
            "application/json"
        );
        assert_eq!(*received.properties.delivery_mode(), Some(2));
        assert_eq!(
            received.properties.message_id().as_ref().unwrap().as_str(),
            event.id().to_string()
        );
        received.ack(BasicAckOptions::default()).await.unwrap();
        case_channel
            .queue_unbind(
                owned.queue.clone().into(),
                owned.exchange.clone().into(),
                owned.routing_key.clone().into(),
                FieldTable::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            publisher.publish(&event).await.unwrap_err().kind(),
            PublishErrorKind::Unroutable
        );
        // Routed confirms and mandatory returns never regenerate identity on repetition.
        publisher.close().await.unwrap();
    });
    let result = tokio::time::timeout(Duration::from_secs(45), &mut case).await;
    if result.is_err() {
        case.abort();
        let _ = case.await;
    }
    // Only objects named by this exact test are deleted. No shared purge/restart.
    tokio::time::timeout(Duration::from_secs(10), async {
        channel
            .queue_delete(topology.queue.into(), QueueDeleteOptions::default())
            .await
            .unwrap();
        channel
            .exchange_delete(topology.exchange.into(), ExchangeDeleteOptions::default())
            .await
            .unwrap();
        connection.close(200, "test cleanup".into()).await.unwrap();
    })
    .await
    .unwrap();
    result.unwrap().unwrap();
}

/// Withhold broker-to-client bytes after setup on a test-owned TCP proxy.
/// This changes no shared broker state or other clients' connections.
async fn proxy() -> (
    String,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    tokio::task::JoinHandle<()>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut url = url::Url::parse(&uri()).unwrap();
    let target = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(5672));
    url.set_host(Some("127.0.0.1")).unwrap();
    url.set_port(Some(address.port())).unwrap();
    let hold = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = hold.clone();
    let task = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let broker = tokio::net::TcpStream::connect(target).await.unwrap();
        let (mut client_read, mut client_write) = client.into_split();
        let (mut broker_read, mut broker_write) = broker.into_split();
        let outgoing = async {
            let _ = tokio::io::copy(&mut client_read, &mut broker_write).await;
        };
        let incoming = async {
            let mut buffer = [0; 8192];
            loop {
                let size = broker_read.read(&mut buffer).await.unwrap_or(0);
                if size == 0 {
                    break;
                }
                if flag.load(std::sync::atomic::Ordering::Acquire) {
                    std::future::pending::<()>().await;
                }
                if client_write.write_all(&buffer[..size]).await.is_err() {
                    break;
                }
            }
        };
        tokio::select! { () = outgoing => {}, () = incoming => {} }
    });
    (url.into(), hold, task)
}

#[tokio::test]
#[ignore = "requires a real RabbitMQ broker and explicit credentials"]
async fn withheld_confirmation_timeout_and_cancellation_are_uncertain_without_retry() {
    use std::sync::{Arc, atomic::Ordering};
    let connection = tokio::time::timeout(
        Duration::from_secs(10),
        Connection::connect(&uri(), ConnectionProperties::default()),
    )
    .await
    .unwrap()
    .unwrap();
    let channel = connection.create_channel().await.unwrap();
    for cancel in [false, true] {
        let id = uuid::Uuid::now_v7().simple().to_string();
        let topology = Topology {
            exchange: format!("relay.it.{id}.exchange"),
            queue: format!("relay.it.{id}.queue"),
            routing_key: "test".into(),
        };
        let (uri, hold, proxy_task) = proxy().await;
        let owned = topology.clone();
        let reader = channel.clone();
        let mut case = tokio::spawn(async move {
            let publisher = Arc::new(
                RabbitMqPublisher::connect(
                    RabbitMqConfig::new(
                        uri,
                        owned.clone(),
                        Duration::from_secs(2),
                        Duration::from_millis(250),
                    )
                    .unwrap(),
                )
                .await
                .unwrap(),
            );
            hold.store(true, Ordering::Release);
            let event = EventEnvelope::new(
                "test",
                "test",
                "1",
                1,
                serde_json::json!({"exact": "value"}),
            )
            .unwrap();
            let sender = publisher.clone();
            let sent = event.clone();
            let publication = tokio::spawn(async move { sender.publish(&sent).await });
            let received = tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if let Some(message) = reader
                        .basic_get(owned.queue.clone().into(), BasicGetOptions::default())
                        .await
                        .unwrap()
                    {
                        break message;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                serde_json::from_slice::<EventEnvelope>(&received.data).unwrap(),
                event
            );
            received.ack(BasicAckOptions::default()).await.unwrap();
            if cancel {
                publication.abort();
                assert!(publication.await.unwrap_err().is_cancelled());
            } else {
                let error = publication.await.unwrap().unwrap_err();
                assert_eq!(error.kind(), PublishErrorKind::Uncertain);
                use std::error::Error;
                assert!(
                    error
                        .source()
                        .unwrap()
                        .downcast_ref::<tokio::time::error::Elapsed>()
                        .is_some()
                );
            }
            assert_eq!(
                publisher.publish(&event).await.unwrap_err().kind(),
                PublishErrorKind::Unavailable
            );
            assert!(
                reader
                    .basic_get(owned.queue.into(), BasicGetOptions::default())
                    .await
                    .unwrap()
                    .is_none()
            );
            // Proxy deliberately withholds close responses too; dropping releases owned I/O.
            drop(publisher);
        });
        let result = tokio::time::timeout(Duration::from_secs(10), &mut case).await;
        if result.is_err() {
            case.abort();
            let _ = case.await;
        }
        proxy_task.abort();
        let _ = proxy_task.await;
        tokio::time::timeout(Duration::from_secs(10), async {
            channel
                .queue_delete(topology.queue.into(), QueueDeleteOptions::default())
                .await
                .unwrap();
            channel
                .exchange_delete(topology.exchange.into(), ExchangeDeleteOptions::default())
                .await
                .unwrap();
        })
        .await
        .unwrap();
        result.unwrap().unwrap();
    }
    connection.close(200, "test cleanup".into()).await.unwrap();
}
