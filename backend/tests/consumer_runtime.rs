//! Opt-in real consumer tests: exact test-owned queues/exchanges and disposable databases.
#[path = "support/faults.rs"]
mod faults;
mod support;
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties,
    options::*,
    types::{AMQPValue, FieldTable},
};
use reliable_event_relay::{
    application::{consume::*, consumer::*, publisher::PublishErrorKind},
    domain::{consumer::ConsumerName, event::EventEnvelope},
    infrastructure::{postgres::PostgresInventoryProcessor, rabbitmq::*},
};
use std::{future::Future, time::Duration};
use support::{TestResult, checked, isolated};
use tokio::time::timeout;
const WAIT: Duration = Duration::from_secs(6);
fn name() -> ConsumerName {
    ConsumerName::new("inventory.stock.v1").unwrap()
}
fn event() -> EventEnvelope {
    EventEnvelope::new(
        "inventory.units_received",
        "stock",
        "1",
        1,
        serde_json::json!({"warehouse_id":"local","sku":"SKU-1","quantity":5}),
    )
    .unwrap()
}
fn config(uri: String, topology: Topology) -> RabbitMqConfig {
    RabbitMqConfig::new(
        uri,
        topology,
        Duration::from_secs(2),
        Duration::from_millis(500),
    )
    .unwrap()
}
async fn schema(pool: &sqlx::PgPool) {
    checked(
        sqlx::raw_sql(include_str!(
            "../migrations/20261014000000_add_atomic_consumer.up.sql"
        ))
        .execute(pool)
        .await,
        "consumer schema",
    );
}
#[derive(Clone)]
struct Broker {
    channel: Channel,
    source: Topology,
    quarantine: Topology,
    uri: String,
}
impl Broker {
    fn consumer(&self, prefetch: u16) -> RabbitConsumerConfig {
        RabbitConsumerConfig::new(
            config(self.uri.clone(), self.source.clone()),
            self.quarantine.clone(),
            prefetch,
        )
        .unwrap()
    }
    async fn send(&self, body: &[u8], properties: BasicProperties) {
        let result = self
            .channel
            .basic_publish(
                self.source.exchange.clone().into(),
                self.source.routing_key.clone().into(),
                BasicPublishOptions {
                    mandatory: true,
                    ..Default::default()
                },
                body,
                properties,
            )
            .await
            .expect("test publication")
            .await
            .expect("test confirm");
        assert!(matches!(result, Confirmation::Ack(None)));
    }
    async fn send_event(&self, e: &EventEnvelope) {
        self.send(
            &serde_json::to_vec(e).unwrap(),
            BasicProperties::default()
                .with_delivery_mode(2)
                .with_message_id(e.id().to_string().into()),
        )
        .await;
    }
    async fn get(&self, queue: &str) -> lapin::message::BasicGetMessage {
        timeout(WAIT, async {
            loop {
                if let Some(m) = self
                    .channel
                    .basic_get(queue.into(), BasicGetOptions::default())
                    .await
                    .expect("test get")
                {
                    break m;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("message observation deadline")
    }
    async fn empty(&self, queue: &str) {
        assert!(
            self.channel
                .basic_get(queue.into(), BasicGetOptions::default())
                .await
                .expect("test empty get")
                .is_none()
        );
    }
}
async fn broker<F, Fut>(case: F)
where
    F: FnOnce(Broker) -> Fut + Send + 'static,
    Fut: Future<Output = TestResult> + Send + 'static,
{
    let uri = faults::uri();
    let connection = timeout(
        WAIT,
        Connection::connect(&uri, ConnectionProperties::default()),
    )
    .await
    .unwrap()
    .expect("test broker connection");
    let channel = connection
        .create_channel()
        .await
        .expect("test observer channel");
    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await
        .expect("test confirms");
    let id = uuid::Uuid::now_v7().simple().to_string();
    let make = |kind: &str| Topology {
        exchange: format!("relay.it.{id}.{kind}.exchange"),
        queue: format!("relay.it.{id}.{kind}.queue"),
        routing_key: kind.to_string(),
    };
    let source = make("source");
    let quarantine = make("quarantine");
    for topology in [&source, &quarantine] {
        RabbitMqPublisher::connect(config(uri.clone(), topology.clone()))
            .await
            .unwrap()
            .close()
            .await
            .unwrap();
    }
    let fixture = Broker {
        channel: channel.clone(),
        source: source.clone(),
        quarantine: quarantine.clone(),
        uri,
    };
    let mut task = tokio::spawn(case(fixture));
    let result = timeout(Duration::from_secs(32), &mut task).await;
    if result.is_err() {
        task.abort();
        let _ = task.await;
    }
    timeout(WAIT, async {
        for topology in [&source, &quarantine] {
            channel
                .queue_delete(topology.queue.clone().into(), QueueDeleteOptions::default())
                .await
                .expect("exact queue cleanup");
            channel
                .exchange_delete(
                    topology.exchange.clone().into(),
                    ExchangeDeleteOptions::default(),
                )
                .await
                .expect("exact exchange cleanup");
            let probe = connection.create_channel().await.expect("cleanup probe");
            let error = probe
                .queue_declare(
                    topology.queue.clone().into(),
                    QueueDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .expect_err("queue must be absent");
            assert!(matches!(error.kind(),lapin::ErrorKind::ProtocolError(e) if e.get_id()==404));
            let probe = connection.create_channel().await.expect("exchange probe");
            let error = probe
                .exchange_declare(
                    topology.exchange.clone().into(),
                    lapin::ExchangeKind::Direct,
                    ExchangeDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .expect_err("exchange must be absent");
            assert!(matches!(error.kind(),lapin::ErrorKind::ProtocolError(e) if e.get_id()==404));
        }
        connection
            .close(200, "exact test cleanup".into())
            .await
            .expect("test connection close");
    })
    .await
    .expect("broker cleanup deadline");
    result
        .expect("consumer case deadline")
        .expect("consumer case assertions")
        .expect("consumer case error (source redacted)");
}
async fn units(pool: &sqlx::PgPool) -> i64 {
    checked(
        sqlx::query_scalar("SELECT COALESCE(sum(units),0)::bigint FROM consumer.stock")
            .fetch_one(pool)
            .await,
        "stock observation",
    )
}
async fn receipts(pool: &sqlx::PgPool) -> i64 {
    checked(
        sqlx::query_scalar("SELECT count(*) FROM consumer.receipts")
            .fetch_one(pool)
            .await,
        "receipt observation",
    )
}
fn record(message: &lapin::message::BasicGetMessage) -> serde_json::Value {
    assert_eq!(*message.properties.delivery_mode(), Some(2));
    assert!(message.properties.expiration().is_none());
    let value = message
        .properties
        .headers()
        .as_ref()
        .unwrap()
        .inner()
        .get("relay-preservation-v1")
        .unwrap();
    let AMQPValue::LongString(bytes) = value else {
        panic!("sidecar type")
    };
    serde_json::from_slice(bytes.as_bytes()).unwrap()
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn real_effect_duplicate_conflict_and_invalid_preservation() {
    isolated(|pool| async move {
        schema(&pool).await;
        checked(
            sqlx::query("INSERT INTO consumer.stock VALUES('local','SKU-1',10)")
                .execute(&pool)
                .await,
            "initial stock",
        );
        broker(move |b| async move {
            let p = PostgresInventoryProcessor::new(pool.clone());
            let mut session = RabbitConsumerSession::connect(b.consumer(1)).await?;
            let a = event();
            for duplicate in [false, true] {
                b.send_event(&a).await;
                let d = timeout(WAIT, session.next()).await??.unwrap();
                let out = handle_delivery(&d, &p, &session.quarantine, &name(), WAIT).await;
                assert!(!out.recover());
                assert!(
                    matches!(
                        out.primary,
                        PrimaryOutcome::Processing(Ok(ProcessingOutcome::Duplicate(_)))
                    ) == duplicate
                );
                assert_eq!(units(&pool).await, 15);
                assert_eq!(receipts(&pool).await, 1);
            }
            let conflict = a.clone().with_correlation_id(uuid::Uuid::now_v7());
            b.send_event(&conflict).await;
            let d = timeout(WAIT, session.next()).await??.unwrap();
            let out = handle_delivery(&d, &p, &session.quarantine, &name(), WAIT).await;
            assert!(matches!(
                out.primary,
                PrimaryOutcome::Processing(Ok(ProcessingOutcome::IdentityConflict))
            ));
            assert!(!out.recover());
            let q = b.get(&b.quarantine.queue).await;
            assert_eq!(q.data, serde_json::to_vec(&conflict)?);
            assert_eq!(record(&q)["reason"], "IdentityConflict");
            assert!(q.ack(BasicAckOptions::default()).await?);
            for unsupported in [false, true] {
                let bytes = if unsupported {
                    serde_json::to_vec(
                        &EventEnvelope::new(
                            "unsupported",
                            "stock",
                            "1",
                            1,
                            serde_json::Value::Null,
                        )
                        .unwrap(),
                    )?
                } else {
                    vec![0xff, 0, 1]
                };
                let mut headers = FieldTable::default();
                headers.insert(
                    "relay-preservation-v1".into(),
                    AMQPValue::LongString("forged authority".into()),
                );
                headers.insert("binary".into(), AMQPValue::ByteArray(vec![0, 255].into()));
                let id = if unsupported {
                    serde_json::from_slice::<EventEnvelope>(&bytes)?
                        .id()
                        .to_string()
                } else {
                    "malformed-id".into()
                };
                let properties = BasicProperties::default()
                    .with_headers(headers)
                    .with_message_id(id.into())
                    .with_expiration("60000".into());
                b.send(&bytes, properties.clone()).await;
                let d = timeout(WAIT, session.next()).await??.unwrap();
                let out = handle_delivery(&d, &p, &session.quarantine, &name(), WAIT).await;
                assert!(!out.recover());
                let q = b.get(&b.quarantine.queue).await;
                assert_eq!(q.data, bytes);
                let sidecar = record(&q);
                assert_eq!(sidecar["format_version"], 1);
                assert_eq!(sidecar["consumer"], name().as_str());
                assert_eq!(sidecar["source_exchange"], b.source.exchange);
                assert_eq!(
                    sidecar["reason"],
                    if unsupported {
                        "Unsupported"
                    } else {
                        "MalformedJson"
                    }
                );
                let restored = decode_properties(&sidecar["original_properties"]).unwrap();
                assert_eq!(restored, properties);
                assert!(q.ack(BasicAckOptions::default()).await?);
            }
            assert_eq!(units(&pool).await, 15);
            assert_eq!(receipts(&pool).await, 1);
            session.close().await?;
            b.empty(&b.source.queue).await;
            b.empty(&b.quarantine.queue).await;
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn mandatory_return_and_known_database_failure_retain_original() {
    isolated(|pool|async move {schema(&pool).await;
        broker(move |b|async move {
            let p=PostgresInventoryProcessor::new(pool.clone());let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;
            b.channel.queue_unbind(b.quarantine.queue.clone().into(),b.quarantine.exchange.clone().into(),b.quarantine.routing_key.clone().into(),FieldTable::default()).await?;
            b.send(b"invalid",BasicProperties::default()).await;let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&p,&session.quarantine,&name(),WAIT).await;
            assert!(out.recover());assert!(out.acknowledgement.is_none());assert_eq!(out.preservation.unwrap().unwrap_err().kind(),PublishErrorKind::Unroutable);
            session.close().await?;let retained=b.get(&b.source.queue).await;assert_eq!(retained.data,b"invalid");assert!(retained.redelivered);assert!(retained.ack(BasicAckOptions::default()).await?);
            let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;
            checked(sqlx::raw_sql("CREATE FUNCTION consumer.reject_test() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_test BEFORE INSERT ON consumer.receipts FOR EACH ROW EXECUTE FUNCTION consumer.reject_test();").execute(&pool).await,"test rejection fixture");
            let a=event();b.send_event(&a).await;let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&p,&session.quarantine,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Err(e)) if e.kind()==ProcessingFailureKind::PreCommit));assert!(out.preservation.is_none());assert!(out.acknowledgement.is_none());session.close().await?;
            let retained=b.get(&b.source.queue).await;assert_eq!(serde_json::from_slice::<EventEnvelope>(&retained.data)?,a);assert!(retained.ack(BasicAckOptions::default()).await?);assert_eq!(receipts(&pool).await,0);assert_eq!(units(&pool).await,0);Ok(())
        }).await;Ok(())
    }).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn lost_database_commit_ack_redelivery_has_one_effect() {
    isolated(|pool|async move {schema(&pool).await;
        broker(move |b|async move {
            let (proxy,server)=faults::lost_commit_ack_pool(&pool).await;let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;let a=event();b.send_event(&a).await;
            let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&PostgresInventoryProcessor::new(proxy.clone()),&session.quarantine,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Err(e)) if e.kind()==ProcessingFailureKind::CommitUncertain));assert!(out.acknowledgement.is_none());assert!(out.preservation.is_none());assert!(timeout(WAIT,server).await??);proxy.close().await;session.close().await?;
            assert_eq!(units(&pool).await,5);assert_eq!(receipts(&pool).await,1);
            let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&PostgresInventoryProcessor::new(pool.clone()),&session.quarantine,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Ok(ProcessingOutcome::Duplicate(_)))));assert!(!out.recover());session.close().await?;b.empty(&b.source.queue).await;assert_eq!(units(&pool).await,5);Ok(())
        }).await;Ok(())
    }).await;
}

// Synchronous abort guard owns only this test's proxy task; no asynchronous Drop cleanup.
struct ProxyTask(tokio::task::JoinHandle<()>);
impl Drop for ProxyTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn lost_quarantine_confirmation_retains_source_and_allows_duplicate_preservation() {
    isolated(|pool| async move {
        schema(&pool).await;
        broker(move |b| async move {
            let mut session = RabbitConsumerSession::connect(b.consumer(1)).await?;
            let (uri, hold, task) = faults::proxy().await;
            let _proxy = ProxyTask(task);
            let quarantine =
                RabbitQuarantinePublisher::connect(config(uri, b.quarantine.clone())).await?;
            hold.store(true, std::sync::atomic::Ordering::Release);
            b.send(b"invalid JSON", BasicProperties::default()).await;
            let d = timeout(WAIT, session.next()).await??.unwrap();
            let out = handle_delivery(
                &d,
                &PostgresInventoryProcessor::new(pool.clone()),
                &quarantine,
                &name(),
                WAIT,
            )
            .await;
            assert_eq!(
                out.preservation.unwrap().unwrap_err().kind(),
                PublishErrorKind::Uncertain
            );
            assert!(out.acknowledgement.is_none());
            assert!(!quarantine.is_usable());
            let copy = b.get(&b.quarantine.queue).await;
            assert_eq!(copy.data, b"invalid JSON");
            assert!(copy.ack(BasicAckOptions::default()).await?);
            drop(_proxy);
            let _ = quarantine.close().await;
            session.close().await?;
            let mut session = RabbitConsumerSession::connect(b.consumer(1)).await?;
            let d = timeout(WAIT, session.next()).await??.unwrap();
            let out = handle_delivery(
                &d,
                &PostgresInventoryProcessor::new(pool.clone()),
                &session.quarantine,
                &name(),
                WAIT,
            )
            .await;
            assert!(!out.recover());
            let copy = b.get(&b.quarantine.queue).await;
            assert_eq!(copy.data, b"invalid JSON");
            assert!(copy.ack(BasicAckOptions::default()).await?);
            session.close().await?;
            b.empty(&b.source.queue).await;
            assert_eq!(receipts(&pool).await, 0);
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn unavailable_database_and_poisoned_source_ack_keep_primary_and_recover() {
    isolated(|pool|async move {schema(&pool).await;
        broker(move |b|async move {
            let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;let a=event();b.send_event(&a).await;
            let closed=sqlx::postgres::PgPoolOptions::new().connect_lazy_with((*pool.connect_options()).clone());closed.close().await;
            let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&PostgresInventoryProcessor::new(closed),&session.quarantine,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Err(e)) if e.kind()==ProcessingFailureKind::Unavailable));assert!(out.acknowledgement.is_none());session.close().await?;
            let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;let d=timeout(WAIT,session.next()).await??.unwrap();session.close().await?;
            let q=RabbitQuarantinePublisher::connect(config(b.uri.clone(),b.quarantine.clone())).await?;
            let out=handle_delivery(&d,&PostgresInventoryProcessor::new(pool.clone()),&q,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Ok(ProcessingOutcome::Applied(_)))));assert!(out.acknowledgement.unwrap().is_err());q.close().await?;
            let mut session=RabbitConsumerSession::connect(b.consumer(1)).await?;let d=timeout(WAIT,session.next()).await??.unwrap();let out=handle_delivery(&d,&PostgresInventoryProcessor::new(pool.clone()),&session.quarantine,&name(),WAIT).await;
            assert!(matches!(out.primary,PrimaryOutcome::Processing(Ok(ProcessingOutcome::Duplicate(_)))));assert!(!out.recover());session.close().await?;b.empty(&b.source.queue).await;assert_eq!(units(&pool).await,5);assert_eq!(receipts(&pool).await,1);Ok(())
        }).await;Ok(())
    }).await;
}
async fn consumers(b: &Broker, expected: u32) {
    timeout(WAIT, async {
        loop {
            let queue = b
                .channel
                .queue_declare(
                    b.source.queue.clone().into(),
                    QueueDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .expect("consumer count");
            if queue.consumer_count() == expected {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("subscription observation deadline");
}
fn command(b: &Broker, database: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_consumer"));
    command
        .env("POSTGRES_DB", database)
        .env("CONSUMER_SOURCE_EXCHANGE", &b.source.exchange)
        .env("CONSUMER_SOURCE_QUEUE", &b.source.queue)
        .env("CONSUMER_SOURCE_ROUTING_KEY", &b.source.routing_key)
        .env("CONSUMER_QUARANTINE_EXCHANGE", &b.quarantine.exchange)
        .env("CONSUMER_QUARANTINE_QUEUE", &b.quarantine.queue)
        .env("CONSUMER_QUARANTINE_ROUTING_KEY", &b.quarantine.routing_key)
        .env("CONSUMER_NAME", name().as_str())
        .env("CONSUMER_PREFETCH", "1")
        .env("CONSUMER_OPERATION_MS", "2000")
        .env("CONSUMER_RECONNECT_MS", "200")
        .env("CONSUMER_DRAIN_MS", "500")
        .env("CONSUMER_SHUTDOWN_MS", "2000")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    command
}
async fn terminate(child: &mut tokio::process::Child) {
    assert!(
        tokio::process::Command::new("kill")
            .args(["-TERM", &child.id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    assert!(
        timeout(Duration::from_secs(4), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn compiled_consumer_prefetch_drain_and_buffered_recovery() {
    isolated(|pool|async move {schema(&pool).await;
        broker(move |b|async move {
            let database:String=checked(sqlx::query_scalar("SELECT current_database()").fetch_one(&pool).await,"test database");
            checked(sqlx::query("INSERT INTO consumer.stock VALUES('local','SKU-1',10)").execute(&pool).await,"initial stock");
            let mut blocker=pool.begin().await?;checked(sqlx::query("SELECT units FROM consumer.stock FOR UPDATE").execute(&mut *blocker).await,"hold effect");
            let observer=sqlx::postgres::PgPoolOptions::new().max_connections(1).connect_with((*pool.connect_options()).clone()).await?;
            let mut child=command(&b,&database).env("CONSUMER_PREFETCH","2").spawn()?;consumers(&b,1).await;
            let a=event();let second=event();b.send_event(&a).await;b.send_event(&second).await;b.send_event(&event()).await;
            timeout(WAIT,async {loop {
                let waiting:bool=checked(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").fetch_one(&observer).await,"consumer effect barrier");
                if waiting {break;}tokio::task::yield_now().await;
            }}).await?;
            // Credit 2 admits one active operation and buffers another; third stays ready.
            timeout(WAIT,async {loop {
                let queue=b.channel.queue_declare(b.source.queue.clone().into(),QueueDeclareOptions{passive:true,..Default::default()},FieldTable::default()).await.expect("prefetch observation");
                if queue.message_count()==1 {break;}tokio::task::yield_now().await;
            }}).await?;
            let waits:i64=checked(sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'").fetch_one(&observer).await,"one in-flight operation");
            assert_eq!(waits,1);
            terminate(&mut child).await;consumers(&b,0).await;
            checked(blocker.rollback().await,"release effect barrier");observer.close().await;
            // Cancelled processing is reconciled by the next real subscription, never by ACK.
            let mut child=command(&b,&database).spawn()?;
            timeout(WAIT,async {loop {if receipts(&pool).await==3 {break;}tokio::task::yield_now().await;}}).await?;
            terminate(&mut child).await;assert_eq!(units(&pool).await,25);b.empty(&b.source.queue).await;Ok(())
        }).await;Ok(())
    }).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn runtime_resubscribes_after_server_cancellation_and_shutdown_idle() {
    isolated(|pool| async move {
        schema(&pool).await;
        broker(move |b| async move {
            let database: String = checked(
                sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(&pool)
                    .await,
                "test database",
            );
            let mut child = command(&b, &database).spawn()?;
            consumers(&b, 1).await;
            // Delete/recreate only this test's source queue: server cancels the subscription.
            b.channel
                .queue_delete(b.source.queue.clone().into(), QueueDeleteOptions::default())
                .await?;
            // Runtime declaration recreates this test-owned queue and binding after pacing.
            timeout(WAIT, async {
                loop {
                    let probe = b.channel.status().connected();
                    assert!(probe);
                    // Probe existence on an independent channel: passive 404 closes only the probe.
                    let connection = Connection::connect(&b.uri, ConnectionProperties::default())
                        .await
                        .expect("probe connection");
                    let channel = connection.create_channel().await.expect("probe channel");
                    let result = channel
                        .queue_declare(
                            b.source.queue.clone().into(),
                            QueueDeclareOptions {
                                passive: true,
                                ..Default::default()
                            },
                            FieldTable::default(),
                        )
                        .await;
                    let ready = result.is_ok_and(|q| q.consumer_count() == 1);
                    let _ = connection.close(200, "probe done".into()).await;
                    if ready {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            b.send_event(&event()).await;
            timeout(WAIT, async {
                loop {
                    if receipts(&pool).await == 1 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            terminate(&mut child).await;
            consumers(&b, 0).await;
            assert_eq!(units(&pool).await, 5);
            b.empty(&b.source.queue).await;
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn incompatible_source_declaration_is_sanitized_and_keeps_existing_queue() {
    isolated(|pool| async move {
        schema(&pool).await;
        broker(move |b| async move {
            // A durable exchange of a different type rejects declaration, without deletion.
            b.channel
                .exchange_delete(
                    b.source.exchange.clone().into(),
                    ExchangeDeleteOptions::default(),
                )
                .await?;
            b.channel
                .exchange_declare(
                    b.source.exchange.clone().into(),
                    lapin::ExchangeKind::Fanout,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            let error = RabbitConsumerSession::connect(b.consumer(1))
                .await
                .err()
                .expect("incompatible topology must fail");
            assert_eq!(error.kind(), PublishErrorKind::Unavailable);
            assert!(
                !format!("{error} {error:?}")
                    .contains(&std::env::var("RABBITMQ_DEFAULT_PASS").unwrap())
            );
            let queue = b
                .channel
                .queue_declare(
                    b.source.queue.clone().into(),
                    QueueDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            assert_eq!(queue.consumer_count(), 0);
            let probe = b
                .channel
                .exchange_declare(
                    b.source.exchange.clone().into(),
                    lapin::ExchangeKind::Fanout,
                    ExchangeDeclareOptions {
                        passive: true,
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await;
            assert!(probe.is_ok());
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL, RabbitMQ and Supervisor in app"]
async fn isolated_supervisor_consumer_is_opt_in_and_named_start_stop_works() {
    isolated(|pool| async move {
        schema(&pool).await;
        broker(move |b| async move {
            let database: String = checked(
                sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(&pool)
                    .await,
                "test database",
            );
            let temp = std::env::temp_dir().join(format!(
                "relay_consumer_supervisor_{}",
                uuid::Uuid::now_v7().simple()
            ));
            std::fs::create_dir(&temp)?;
            let path = temp.join("supervisord.conf");
            let physical = include_str!("../../docker/supervisor/normal.conf")
                .replace("/tmp/relay-supervisor", temp.to_str().unwrap())
                .replace(
                    "command=/usr/local/bin/relay-consumer",
                    &format!("command={}", env!("CARGO_BIN_EXE_consumer")),
                )
                .replace("autostart=true", "autostart=false");
            std::fs::write(&path, physical)?;
            let child = command(&b, &database); // Reuse the exact isolated environment, never shared defaults.
            let environment: Vec<_> = child
                .as_std()
                .get_envs()
                .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
                .collect();
            let mut manager = tokio::process::Command::new("/usr/bin/supervisord")
                .args(["-c", path.to_str().unwrap()])
                .envs(environment)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()?;
            let case = async {
                timeout(WAIT, async {
                    while !temp.join("control.sock").exists() {
                        tokio::task::yield_now().await;
                    }
                })
                .await?;
                async fn ctl(path: &std::path::Path, args: &[&str]) -> String {
                    let output = timeout(
                        WAIT,
                        tokio::process::Command::new("/usr/bin/supervisorctl")
                            .args(["-c", path.to_str().unwrap()])
                            .args(args)
                            .output(),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert!(output.status.success() || output.status.code() == Some(3));
                    String::from_utf8(output.stdout).unwrap()
                }
                assert!(
                    ctl(&path, &["status", "consumer"])
                        .await
                        .contains("STOPPED")
                );
                consumers(&b, 0).await;
                assert!(ctl(&path, &["start", "consumer"]).await.contains("started"));
                consumers(&b, 1).await;
                b.send_event(&event()).await;
                timeout(WAIT, async {
                    loop {
                        if receipts(&pool).await == 1 {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await?;
                ctl(&path, &["stop", "consumer"]).await;
                assert!(
                    ctl(&path, &["status", "consumer"])
                        .await
                        .contains("STOPPED")
                );
                consumers(&b, 0).await;
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
            .await;
            terminate(&mut manager).await;
            std::fs::remove_dir_all(temp)?;
            case?;
            assert_eq!(units(&pool).await, 5);
            b.empty(&b.source.queue).await;
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RabbitMQ"]
async fn missing_consumer_schema_fails_before_subscription() {
    isolated(|pool| async move {
        broker(move |b| async move {
            let database: String = checked(
                sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(&pool)
                    .await,
                "test database",
            );
            let output = timeout(
                WAIT,
                command(&b, &database)
                    .stderr(std::process::Stdio::piped())
                    .output(),
            )
            .await??;
            assert!(!output.status.success());
            let text = String::from_utf8(output.stderr)?;
            assert!(text.contains("consumer schema unavailable"));
            assert!(text.contains("20261014000000"));
            assert!(!text.contains(&std::env::var("POSTGRES_PASSWORD").unwrap()));
            consumers(&b, 0).await;
            Ok(())
        })
        .await;
        Ok(())
    })
    .await;
}
