//! Real adapters and processes; all instrumentation is confined to this test target.
#[path = "support/faults.rs"]
mod faults;
mod support;
use lapin::{Channel, Connection, ConnectionProperties, options::*};
use reliable_event_relay::{
    application::{
        delivery::{DeliveryOutcome, ReleaseOutcome, deliver_once},
        publisher::{EventPublisher, PublishError, PublishErrorKind},
        worker::{Decision, decision},
    },
    domain::{delivery::LeaseDuration, event::EventEnvelope},
    infrastructure::{
        postgres::PostgresOutboxRepository,
        rabbitmq::{RabbitMqConfig, RabbitMqPublisher, Topology},
    },
    persistence::{
        AcquireRequest, OutboxAcquirer, OutboxCompleter, OutboxReleaser, OwnedEventKey,
        OwnedOutboxEvent, PersistenceError, PersistenceErrorKind, TransitionOutcome,
    },
};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    future::Future,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use support::{checked, isolated};
use tokio::{
    process::{Child, Command},
    time::timeout,
};
use uuid::Uuid;
const WAIT: Duration = Duration::from_secs(8);

fn request(ms: u64) -> AcquireRequest {
    AcquireRequest::new(LeaseDuration::from_micros((ms * 1000) as i64).unwrap())
}
fn config(topology: Topology) -> RabbitMqConfig {
    RabbitMqConfig::new(faults::uri(), topology, WAIT, WAIT).unwrap()
}
async fn sql(pool: &PgPool, sql: &str) {
    checked(sqlx::raw_sql(sql).execute(pool).await, "fixture SQL");
}
async fn seed(pool: &PgPool, id: Uuid) {
    println!("inserted event ID: {id}");
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload) VALUES ($1,'test','test','1',1,clock_timestamp(),'{\"recovery\":true}')").bind(id).execute(pool).await, "seed known event");
}
async fn until(pool: &PgPool, predicate: &str) {
    timeout(WAIT, async {
        loop {
            if checked(
                sqlx::query_scalar::<_, bool>(predicate)
                    .fetch_one(pool)
                    .await,
                "bounded observation",
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("durable predicate deadline");
}
async fn state(pool: &PgPool) -> serde_json::Value {
    checked(sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_object('id',id,'status',status,'count',attempt_count,'token',ownership_token,'acquired',acquired_at,'expires',expires_at,'processed',processed_at) ORDER BY id) FROM relay.outbox_events").fetch_one(pool).await, "durable snapshot")
}
async fn message(channel: &Channel, queue: &str, id: Uuid) {
    let msg = timeout(WAIT, async {
        loop {
            if let Some(msg) = channel
                .basic_get(queue.into(), BasicGetOptions::default())
                .await
                .expect("observer get")
            {
                break msg;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("message deadline");
    let envelope: EventEnvelope = serde_json::from_slice(&msg.data).expect("original envelope");
    assert_eq!(envelope.id(), id);
    assert_eq!(envelope.payload(), &serde_json::json!({"recovery":true}));
    assert_eq!(
        msg.properties.message_id().as_ref().unwrap().as_str(),
        id.to_string()
    );
    println!("observed broker envelope/message identity: {id}");
    msg.ack(BasicAckOptions::default())
        .await
        .expect("observer ACK");
}
async fn empty(channel: &Channel, queue: &str) {
    assert!(
        channel
            .basic_get(queue.into(), BasicGetOptions::default())
            .await
            .expect("observer empty")
            .is_none()
    );
}

struct Resources {
    pool: PgPool,
    topology: Topology,
    channel: Channel,
    temp: PathBuf,
    children: Mutex<Vec<Child>>,
    proxies: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
impl Resources {
    async fn spawn(&self, instrumented: Option<&str>, broker_port: Option<u16>) -> usize {
        let database: String = checked(
            sqlx::query_scalar("SELECT current_database()")
                .fetch_one(&self.pool)
                .await,
            "owned database name",
        );
        let mut cmd = Command::new(if instrumented.is_some() {
            std::env::current_exe().unwrap()
        } else {
            PathBuf::from(env!("CARGO_BIN_EXE_relay"))
        });
        if let Some(mode) = instrumented {
            cmd.args(["--ignored", "--exact", "boundary_child"]);
            cmd.env("RECOVERY_CHILD", mode)
                .env("RECOVERY_READY", self.temp.join("ready"));
        }
        cmd.env("POSTGRES_DB", database)
            .env("RABBITMQ_EXCHANGE", &self.topology.exchange)
            .env("RABBITMQ_QUEUE", &self.topology.queue)
            .env("RABBITMQ_ROUTING_KEY", &self.topology.routing_key)
            .env("RELAY_LEASE_MS", "8000")
            .env("RELAY_IDLE_MS", "20")
            .env("RELAY_RECONNECT_MS", "100")
            .env("RELAY_FAILURE_MS", "100")
            .env("RELAY_DRAIN_MS", "200")
            .env("RELAY_SHUTDOWN_MS", "2000")
            .env("RELAY_LOCK_MS", "2000")
            .env("RELAY_STATEMENT_MS", "1000")
            .env("RABBITMQ_CONNECT_TIMEOUT_MS", "200")
            .env("RABBITMQ_PUBLISH_TIMEOUT_MS", "500")
            .stdout(Stdio::from(
                std::fs::File::create(
                    self.temp
                        .join(format!("child-{}.log", self.children.lock().unwrap().len())),
                )
                .unwrap(),
            ))
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(port) = broker_port {
            cmd.env("RABBITMQ_HOST", "127.0.0.1")
                .env("RABBITMQ_PORT", port.to_string());
        }
        let child = cmd.spawn().expect("spawn owned child");
        let mut children = self.children.lock().unwrap();
        children.push(child);
        children.len() - 1
    }
    async fn stop(&self, index: usize, signal: &str) {
        // Remove the handle before await; the task keeps exclusive ownership.
        if signal != "-KILL" {
            self.log_until(index, "relay started").await;
        }
        let mut child = self.children.lock().unwrap().remove(index);
        let pid = child.id().expect("live owned child");
        let status = Command::new("kill")
            .args([signal, &pid.to_string()])
            .status()
            .await
            .expect("signal owned PID");
        if !status.success() {
            child
                .kill()
                .await
                .expect("failed signal fallback kill/reap");
        }
        let exit = timeout(Duration::from_secs(4), child.wait()).await;
        if exit.is_err() {
            child.kill().await.expect("fallback kill/reap");
        }
        let exit = exit.expect("bounded child exit").expect("reap child");
        assert!(status.success());
        if signal != "-KILL" {
            assert!(exit.success());
        }
    }
    async fn log_until(&self, index: usize, needle: &str) {
        timeout(WAIT, async {
            loop {
                let log = std::fs::read_to_string(self.temp.join(format!("child-{index}.log")))
                    .unwrap_or_default();
                if log.contains(needle) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("sanitized child log barrier");
    }
    async fn ready(&self) {
        timeout(WAIT, async {
            while !self.temp.join("ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("explicit process barrier");
    }
}
async fn experiment<F, Fut>(case: F)
where
    F: FnOnce(Arc<Resources>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    isolated(move |pool| async move {
        sql(
            &pool,
            include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
        )
        .await;
        let name = format!("relay.it.{}", Uuid::now_v7().simple());
        let topology = Topology {
            exchange: format!("{name}.exchange"),
            queue: format!("{name}.queue"),
            routing_key: "test".into(),
        };
        println!("owned broker: {} {}", topology.exchange, topology.queue);
        let connection = timeout(
            WAIT,
            Connection::connect(&faults::uri(), ConnectionProperties::default()),
        )
        .await
        .unwrap()
        .expect("observer connect");
        let channel = connection.create_channel().await.expect("observer channel");
        let temp = std::env::temp_dir().join(format!("relay_recovery_{}", Uuid::now_v7().simple()));
        std::fs::create_dir(&temp).unwrap();
        let resources = Arc::new(Resources {
            pool,
            topology,
            channel,
            temp,
            children: Mutex::new(Vec::new()),
            proxies: Mutex::new(Vec::new()),
        });
        let r = resources.clone();
        let mut task = tokio::spawn(async move {
            let publisher = RabbitMqPublisher::connect(config(r.topology.clone()))
                .await
                .expect("owned topology");
            publisher.close().await.expect("topology setup close");
            case(r).await;
        });
        let result = timeout(Duration::from_secs(30), &mut task).await;
        if result.is_err() {
            task.abort();
            let _ = task.await;
        }
        println!(
            "final durable snapshot (no payload): {}",
            state(&resources.pool).await
        );
        let children = std::mem::take(&mut *resources.children.lock().unwrap());
        for mut child in children {
            if child.try_wait().unwrap().is_none() {
                child.kill().await.expect("cleanup child kill/reap");
            }
        }
        let proxies = std::mem::take(&mut *resources.proxies.lock().unwrap());
        for proxy in proxies {
            proxy.abort();
            let _ = proxy.await;
        }
        timeout(WAIT, async {
            resources
                .channel
                .queue_delete(
                    resources.topology.queue.clone().into(),
                    QueueDeleteOptions::default(),
                )
                .await
                .expect("owned queue cleanup");
            resources
                .channel
                .exchange_delete(
                    resources.topology.exchange.clone().into(),
                    ExchangeDeleteOptions::default(),
                )
                .await
                .expect("owned exchange cleanup");
            // Passive declarations must fail on separate disposable channels after deletion.
            let q = connection.create_channel().await.unwrap();
            assert!(
                q.queue_declare(
                    resources.topology.queue.clone().into(),
                    QueueDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    Default::default()
                )
                .await
                .is_err()
            );
            let e = connection.create_channel().await.unwrap();
            assert!(
                e.exchange_declare(
                    resources.topology.exchange.clone().into(),
                    lapin::ExchangeKind::Direct,
                    ExchangeDeclareOptions {
                        passive: true,
                        ..Default::default()
                    },
                    Default::default()
                )
                .await
                .is_err()
            );
            connection
                .close(200, "test cleanup".into())
                .await
                .expect("observer close");
        })
        .await
        .expect("broker cleanup deadline");
        std::fs::remove_dir_all(&resources.temp).unwrap();
        assert!(!resources.temp.exists());
        result
            .expect("experiment deadline")
            .expect("experiment assertions");
        Ok(())
    })
    .await;
}

// This child is a test harness process, not a production fault switch. The same
// deliver_once runs against real adapters; the wrapper stops at one exact boundary.
struct BarrierPublisher {
    inner: RabbitMqPublisher,
    after: bool,
    ready: PathBuf,
}
impl EventPublisher for BarrierPublisher {
    async fn publish(&self, event: &EventEnvelope) -> Result<(), PublishError> {
        if self.after {
            self.inner.publish(event).await?;
        }
        std::fs::write(&self.ready, b"boundary reached").expect("signal barrier");
        std::future::pending().await
    }
}
#[tokio::test]
#[ignore = "internal child entry; no-op without RECOVERY_CHILD"]
async fn boundary_child() {
    let Ok(mode) = std::env::var("RECOVERY_CHILD") else {
        return;
    };
    let options = PgConnectOptions::new()
        .host(&std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres".into()))
        .port(
            std::env::var("POSTGRES_PORT")
                .unwrap_or_else(|_| "5432".into())
                .parse()
                .unwrap(),
        )
        .username(&std::env::var("POSTGRES_USER").unwrap())
        .password(&std::env::var("POSTGRES_PASSWORD").unwrap())
        .database(&std::env::var("POSTGRES_DB").unwrap());
    let pool = checked(
        PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await,
        "child pool",
    );
    let publisher =
        RabbitMqPublisher::connect(RabbitMqConfig::from_lookup(|k| std::env::var(k).ok()).unwrap())
            .await
            .expect("child publisher");
    let barrier = BarrierPublisher {
        inner: publisher,
        after: mode == "after",
        ready: std::env::var("RECOVERY_READY").unwrap().into(),
    };
    deliver_once(
        &PostgresOutboxRepository::new(pool),
        &barrier,
        request(1500),
    )
    .await;
    panic!("barrier must not return");
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn crash_before_send_and_after_confirmation_recover_after_real_expiration() {
    for after in [false, true] {
        experiment(move |r| async move {
            let id=Uuid::now_v7(); seed(&r.pool,id).await;
            r.spawn(Some(if after {"after"} else {"before"}),None).await; r.ready().await;
            let before=state(&r.pool).await;
            assert_eq!(before[0]["status"],"pending"); assert_eq!(before[0]["count"],1); assert!(!before[0]["token"].is_null()); assert!(before[0]["processed"].is_null());
            if after { message(&r.channel,&r.topology.queue,id).await; } else { empty(&r.channel,&r.topology.queue).await; }
            r.stop(0,"-KILL").await;
            assert_eq!(before,state(&r.pool).await);
            until(&r.pool,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
            sql(&r.pool,"CREATE FUNCTION relay.hold_complete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='processed' THEN PERFORM pg_advisory_xact_lock(26); END IF; RETURN NEW; END $$; CREATE TRIGGER hold_complete BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.hold_complete()").await;
            let mut lock=checked(r.pool.acquire().await,"recovery lock connection");
            checked(sqlx::query("SELECT pg_advisory_lock(26)").execute(&mut *lock).await,"recovery barrier");
            r.spawn(None,None).await;
            until(&r.pool,"SELECT status='pending' AND attempt_count=2 AND expires_at>clock_timestamp() FROM relay.outbox_events").await;
            until(&r.pool,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").await;
            let fresh=state(&r.pool).await;assert_ne!(fresh[0]["token"],before[0]["token"]);
            message(&r.channel,&r.topology.queue,id).await;
            checked(sqlx::query("SELECT pg_advisory_unlock(26)").execute(&mut *lock).await,"release recovery barrier");drop(lock);
            until(&r.pool,"SELECT status='processed' AND attempt_count=2 AND ownership_token IS NULL AND acquired_at IS NULL AND expires_at IS NULL FROM relay.outbox_events").await;
            empty(&r.channel,&r.topology.queue).await; r.stop(0,"-TERM").await;
            println!("crash boundary after_confirm={after}: pending count1; elapsed expiry; binary fresh token count2; processed; original ID preserved");
        }).await;
    }
}

// Forwarding repository changes only which real connection executes completion.
struct CompletionConnection {
    direct: PostgresOutboxRepository,
    completion: PostgresOutboxRepository,
}
impl OutboxAcquirer for CompletionConnection {
    async fn acquire(
        &self,
        r: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        self.direct.acquire(r).await
    }
}
impl OutboxCompleter for CompletionConnection {
    async fn complete(&self, k: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.completion.complete(k).await
    }
}
impl OutboxReleaser for CompletionConnection {
    async fn release(&self, k: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.direct.release(k).await
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn confirmed_publication_with_lost_completion_ack_is_not_replayed() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        let (pool,proxy)=faults::lost_commit_ack_pool(&r.pool).await;
        let (observed_tx,observed_rx)=tokio::sync::oneshot::channel();
        // The original task is joined; retain an abort handle for outer cleanup.
        let abort=proxy.abort_handle();
        r.proxies.lock().unwrap().push(tokio::spawn(async move {
            let result=proxy.await;let _=observed_tx.send(result);
        }));
        struct ProxyAbort(tokio::task::AbortHandle);
        impl Drop for ProxyAbort {fn drop(&mut self){self.0.abort();}}
        let _proxy_abort=ProxyAbort(abort);
        let repo=CompletionConnection { direct:PostgresOutboxRepository::new(r.pool.clone()), completion:PostgresOutboxRepository::new(pool.clone()) };
        let publisher=RabbitMqPublisher::connect(config(r.topology.clone())).await.unwrap();
        let outcome=timeout(WAIT,deliver_once(&repo,&publisher,request(8000))).await.unwrap();
        assert!(matches!(outcome,DeliveryOutcome::Published { completion:Err(ref e),.. } if e.kind()==PersistenceErrorKind::CommitUncertain));
        assert!(timeout(WAIT,observed_rx).await.unwrap().unwrap().unwrap()); pool.close().await; publisher.close().await.unwrap();
        let durable=state(&r.pool).await;assert_eq!(durable[0]["status"],"processed");assert_eq!(durable[0]["count"],1);assert!(durable[0]["token"].is_null());
        message(&r.channel,&r.topology.queue,id).await;
        assert!(repo.direct.acquire(request(8000)).await.unwrap().is_none());
        r.spawn(None,None).await;
        // Bounded observation of an actual idle invocation via its transaction
        // count is not needed: direct durable predicate proves ineligibility.
        r.log_until(0,"NoEvent").await;
        r.stop(0,"-INT").await; assert_eq!(durable,state(&r.pool).await);empty(&r.channel,&r.topology.queue).await;
        println!("lost completion ACK: CommitUncertain; independent processed count1; no reacquisition/republication");
    }).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn uncertain_publication_retains_claim_and_fresh_connection_does_not_replay() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        let (uri,hold,proxy)=faults::proxy().await;r.proxies.lock().unwrap().push(proxy);
        let publisher=RabbitMqPublisher::connect(RabbitMqConfig::new(uri,r.topology.clone(),WAIT,Duration::from_millis(250)).unwrap()).await.unwrap();
        hold.store(true,std::sync::atomic::Ordering::Release);
        let repo=PostgresOutboxRepository::new(r.pool.clone());
        let outcome=deliver_once(&repo,&publisher,request(1500)).await;
        assert!(matches!(outcome,DeliveryOutcome::PublicationFailed { ref error,release:ReleaseOutcome::LeaseRetained,.. } if error.kind()==PublishErrorKind::Uncertain));
        assert_eq!(decision(&outcome),Decision::Replace);assert!(!publisher.is_usable());drop(publisher);
        message(&r.channel,&r.topology.queue,id).await; let before=state(&r.pool).await;
        assert_eq!(before[0]["status"],"pending");assert_eq!(before[0]["count"],1);
        let fresh=RabbitMqPublisher::connect(config(r.topology.clone())).await.unwrap();
        assert!(matches!(deliver_once(&repo,&fresh,request(8000)).await,DeliveryOutcome::NoEvent));empty(&r.channel,&r.topology.queue).await;
        until(&r.pool,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
        let outcome=deliver_once(&repo,&fresh,request(8000)).await;
        match outcome { DeliveryOutcome::Published { claim,completion:Ok(TransitionOutcome::Applied) } => { assert_eq!(claim.attempt_count,2);assert_ne!(claim.key.token().get().to_string(),before[0]["token"].as_str().unwrap()); },_=>panic!("recovery must acknowledge completion") }
        message(&r.channel,&r.topology.queue,id).await;fresh.close().await.unwrap();
        println!("uncertain publication: first broker message independently observed; pending retained; fresh connection no replay; expiry count2 processed, duplicate original ID");
    }).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn ordinary_completion_failure_recovers_and_stale_owner_cannot_transition() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        sql(&r.pool,"CREATE FUNCTION relay.reject_complete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='processed' THEN RAISE EXCEPTION 'fixture' USING ERRCODE='23514'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_complete AFTER UPDATE ON relay.outbox_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION relay.reject_complete()").await;
        let repo=PostgresOutboxRepository::new(r.pool.clone());let publisher=RabbitMqPublisher::connect(config(r.topology.clone())).await.unwrap();
        let outcome=deliver_once(&repo,&publisher,request(1500)).await;
        let key=match outcome { DeliveryOutcome::Published { claim,completion:Err(e) } => { assert_eq!(e.kind(),PersistenceErrorKind::OperationFailed);claim.key },_=>panic!("ordinary completion rejection") };
        message(&r.channel,&r.topology.queue,id).await; assert_eq!(state(&r.pool).await[0]["status"],"pending");
        sql(&r.pool,"DROP TRIGGER reject_complete ON relay.outbox_events; DROP FUNCTION relay.reject_complete()").await;
        until(&r.pool,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
        let b=repo.acquire(request(8000)).await.unwrap().unwrap();assert_ne!(key.token(),b.key().token());
        assert_eq!(repo.complete(key).await.unwrap(),TransitionOutcome::OwnershipLost);assert_eq!(repo.release(key).await.unwrap(),TransitionOutcome::OwnershipLost);
        // A stale external sender can still publish the original envelope.
        let original=b.snapshot().event().clone();
        publisher.publish(&original).await.unwrap();message(&r.channel,&r.topology.queue,id).await;
        assert_eq!(state(&r.pool).await[0]["token"],b.key().token().get().to_string());
        publisher.publish(b.snapshot().event()).await.unwrap();assert_eq!(repo.complete(b.key()).await.unwrap(),TransitionOutcome::Applied);
        message(&r.channel,&r.topology.queue,id).await;publisher.close().await.unwrap();
        println!("ordinary completion rejected: pending; expired A transitions rejected; external stale send accepted; B count2 processed after confirm");
    }).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn two_real_workers_have_distinct_durable_owners_before_completion() {
    experiment(|r|async move {
        let ids=[Uuid::now_v7(),Uuid::now_v7()];for id in ids {seed(&r.pool,id).await;}
        sql(&r.pool,"CREATE FUNCTION relay.hold_complete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='processed' THEN PERFORM pg_advisory_xact_lock(26); END IF; RETURN NEW; END $$; CREATE TRIGGER hold_complete BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.hold_complete()").await;
        let mut lock=checked(r.pool.acquire().await,"owned lock connection");checked(sqlx::query("SELECT pg_advisory_lock(26)").execute(&mut *lock).await,"hold fixture barrier");
        r.spawn(None,None).await;r.spawn(None,None).await;
        until(&r.pool,"SELECT count(*)=2 AND count(DISTINCT ownership_token)=2 AND bool_and(status='pending' AND attempt_count=1 AND expires_at>clock_timestamp()) FROM relay.outbox_events").await;
        until(&r.pool,"SELECT count(*)>=2 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'").await;
        let mut observed=Vec::new();
        for _ in ids {
            let msg=r.channel.basic_get(r.topology.queue.clone().into(),BasicGetOptions::default()).await.unwrap().expect("two confirmed messages");
            let envelope:EventEnvelope=serde_json::from_slice(&msg.data).unwrap();
            assert_eq!(msg.properties.message_id().as_ref().unwrap().as_str(),envelope.id().to_string());
            println!("observed broker envelope/message identity: {}",envelope.id());observed.push(envelope.id());msg.ack(BasicAckOptions::default()).await.unwrap();
        }
        observed.sort();let mut expected=ids.to_vec();expected.sort();assert_eq!(observed,expected);
        checked(sqlx::query("SELECT pg_advisory_unlock(26)").execute(&mut *lock).await,"release fixture barrier");drop(lock);
        until(&r.pool,"SELECT bool_and(status='processed' AND attempt_count=1 AND ownership_token IS NULL) FROM relay.outbox_events").await;
        r.stop(0,"-TERM").await;r.stop(0,"-INT").await;empty(&r.channel,&r.topology.queue).await;
        println!("two binaries: two live durable distinct tokens/count1; confirmed messages; barrier release -> processed");
    }).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn real_worker_waits_for_broker_availability_and_shutdown_interrupts_reconnect() {
    experiment(|r|async move {
        use std::sync::atomic::{AtomicBool, Ordering};
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let port=listener.local_addr().unwrap().port();
        let enabled=Arc::new(AtomicBool::new(false));let flag=enabled.clone();
        let upstream=url::Url::parse(&faults::uri()).unwrap();let host=upstream.host_str().unwrap().to_owned();let upstream_port=upstream.port().unwrap_or(5672);
        let proxy=tokio::spawn(async move {
            let mut connections=tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=> {
                        let (mut client,_)=accepted.unwrap();let forward=flag.load(Ordering::Acquire);let host=host.clone();
                        connections.spawn(async move {
                            if forward { let mut broker=tokio::net::TcpStream::connect((host.as_str(),upstream_port)).await.unwrap();let _=tokio::io::copy_bidirectional(&mut client,&mut broker).await; }
                            else { let _held=client;std::future::pending::<()>().await; }
                        });
                    },
                    _=connections.join_next(),if !connections.is_empty()=>{}
                }
            }
        });r.proxies.lock().unwrap().push(proxy);
        r.spawn(None,Some(port)).await;
        r.log_until(0,"publisher connection failed").await;
        assert_eq!(state(&r.pool).await[0]["count"],0);empty(&r.channel,&r.topology.queue).await;
        enabled.store(true,Ordering::Release);
        until(&r.pool,"SELECT status='processed' AND attempt_count=1 AND ownership_token IS NULL FROM relay.outbox_events").await;
        message(&r.channel,&r.topology.queue,id).await;r.log_until(0,"NoEvent").await;r.stop(0,"-TERM").await;
        enabled.store(false,Ordering::Release);
        r.spawn(None,Some(port)).await;r.log_until(0,"publisher connection failed").await;r.stop(0,"-INT").await;
        assert_eq!(state(&r.pool).await[0]["count"],1);
        println!("real binary: unavailable handshake count0; restored proxy processed count1; TERM idle and INT reconnect bounded");
    }).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn graceful_shutdown_cancels_blocked_completion_then_expired_work_recovers() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        sql(&r.pool,"CREATE FUNCTION relay.hold_complete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='processed' THEN PERFORM pg_advisory_xact_lock(26); END IF; RETURN NEW; END $$; CREATE TRIGGER hold_complete BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.hold_complete()").await;
        let mut lock=checked(r.pool.acquire().await,"shutdown lock connection");
        checked(sqlx::query("SELECT pg_advisory_lock(26)").execute(&mut *lock).await,"shutdown completion barrier");
        r.spawn(None,None).await;
        until(&r.pool,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").await;
        message(&r.channel,&r.topology.queue,id).await;
        r.stop(0,"-TERM").await;
        assert_eq!(state(&r.pool).await[0]["status"],"pending");assert_eq!(state(&r.pool).await[0]["count"],1);
        checked(sqlx::query("SELECT pg_advisory_unlock(26)").execute(&mut *lock).await,"unlock after shutdown");drop(lock);
        sql(&r.pool,"DROP TRIGGER hold_complete ON relay.outbox_events; DROP FUNCTION relay.hold_complete()").await;
        until(&r.pool,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
        r.spawn(None,None).await;
        until(&r.pool,"SELECT status='processed' AND attempt_count=2 AND ownership_token IS NULL FROM relay.outbox_events").await;
        message(&r.channel,&r.topology.queue,id).await;r.stop(0,"-TERM").await;
        println!("blocked completion: confirmed first message; TERM drain cancellation pending count1; elapsed expiry restart processed count2");
    }).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn database_lock_deadline_fails_completion_without_premature_processed_state() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        sql(&r.pool,"CREATE FUNCTION relay.hold_complete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status='processed' THEN PERFORM pg_advisory_xact_lock(26); END IF; RETURN NEW; END $$; CREATE TRIGGER hold_complete BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.hold_complete()").await;
        let mut lock=checked(r.pool.acquire().await,"deadline lock connection");checked(sqlx::query("SELECT pg_advisory_lock(26)").execute(&mut *lock).await,"deadline completion barrier");
        let options=(*r.pool.connect_options()).clone().options([("lock_timeout","100"),("statement_timeout","300")]);
        let completion=checked(PgPoolOptions::new().max_connections(1).connect_with(options).await,"deadline completion pool");
        let repo=CompletionConnection { direct:PostgresOutboxRepository::new(r.pool.clone()), completion:PostgresOutboxRepository::new(completion.clone()) };
        let publisher=RabbitMqPublisher::connect(config(r.topology.clone())).await.unwrap();
        let outcome=timeout(Duration::from_secs(2),deliver_once(&repo,&publisher,request(1500))).await.expect("database operation bounded");
        assert!(matches!(outcome,DeliveryOutcome::Published { completion:Err(ref e),.. } if e.kind()==PersistenceErrorKind::OperationFailed));
        message(&r.channel,&r.topology.queue,id).await;assert_eq!(state(&r.pool).await[0]["status"],"pending");
        completion.close().await;publisher.close().await.unwrap();
        checked(sqlx::query("SELECT pg_advisory_unlock(26)").execute(&mut *lock).await,"deadline unlock");drop(lock);
        sql(&r.pool,"DROP TRIGGER hold_complete ON relay.outbox_events; DROP FUNCTION relay.hold_complete()").await;
        until(&r.pool,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
        r.spawn(None,None).await;until(&r.pool,"SELECT status='processed' AND attempt_count=2 FROM relay.outbox_events").await;
        message(&r.channel,&r.topology.queue,id).await;r.stop(0,"-TERM").await;
        println!("database lock deadline: ordinary completion error under 2s, pending; availability restored, elapsed expiry binary processed count2");
    }).await;
}

struct ObservedPublisher {
    inner: RabbitMqPublisher,
    uncertain: Arc<std::sync::atomic::AtomicBool>,
}
impl EventPublisher for ObservedPublisher {
    async fn publish(&self, event: &EventEnvelope) -> Result<(), PublishError> {
        let result = self.inner.publish(event).await;
        if result
            .as_ref()
            .is_err_and(|e| e.kind() == PublishErrorKind::Uncertain)
        {
            self.uncertain
                .store(true, std::sync::atomic::Ordering::Release);
        }
        result
    }
}
struct FaultLifecycle {
    first_uri: String,
    hold: Arc<std::sync::atomic::AtomicBool>,
    topology: Topology,
    connections: std::sync::atomic::AtomicUsize,
    replaced: tokio::sync::Notify,
    uncertain: Arc<std::sync::atomic::AtomicBool>,
}
impl reliable_event_relay::application::worker::PublisherLifecycle for FaultLifecycle {
    type Publisher = ObservedPublisher;
    async fn connect(&self) -> Result<Self::Publisher, PublishError> {
        let number = self
            .connections
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let uri = if number == 0 {
            self.first_uri.clone()
        } else {
            faults::uri()
        };
        let inner = RabbitMqPublisher::connect(
            RabbitMqConfig::new(
                uri,
                self.topology.clone(),
                Duration::from_millis(250),
                Duration::from_millis(2000),
            )
            .unwrap(),
        )
        .await?;
        if number == 0 {
            self.hold.store(true, std::sync::atomic::Ordering::Release);
        } else {
            self.replaced.notify_one();
        }
        Ok(ObservedPublisher {
            inner,
            uncertain: self.uncertain.clone(),
        })
    }
    fn usable(&self, p: &Self::Publisher) -> bool {
        p.inner.is_usable()
    }
    async fn close(&self, p: Self::Publisher) -> Result<(), PublishError> {
        p.inner.close().await
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn integrated_worker_replaces_failed_delivery_connection_before_fresh_acquisition() {
    experiment(|r|async move {
        let id=Uuid::now_v7();seed(&r.pool,id).await;
        let (first_uri,hold,proxy)=faults::proxy().await;let disconnect=proxy.abort_handle();r.proxies.lock().unwrap().push(proxy);
        let lifecycle=FaultLifecycle { first_uri,hold,topology:r.topology.clone(),connections:std::sync::atomic::AtomicUsize::new(0),replaced:tokio::sync::Notify::new(),uncertain:Arc::new(std::sync::atomic::AtomicBool::new(false)) };
        let config=reliable_event_relay::config::WorkerConfig::from_lookup(|key|match key {
            "RELAY_LEASE_MS"=>Some("1500".into()),"RELAY_IDLE_MS"|"RELAY_RECONNECT_MS"|"RELAY_FAILURE_MS"=>Some("20".into()),_=>None,
        }).unwrap();
        let repository=PostgresOutboxRepository::new(r.pool.clone());let (stop_tx,stop_rx)=tokio::sync::oneshot::channel();let mut shutdown=Box::pin(async {let _=stop_rx.await;});
        let observer=async {
            message(&r.channel,&r.topology.queue,id).await;disconnect.abort();
            timeout(WAIT,lifecycle.replaced.notified()).await.expect("real replacement boundary");
            assert!(lifecycle.uncertain.load(std::sync::atomic::Ordering::Acquire));
            let retained=state(&r.pool).await;assert_eq!(retained[0]["status"],"pending");assert_eq!(retained[0]["count"],1);
            assert!(!retained[0]["token"].is_null());
            let live:bool=checked(sqlx::query_scalar("SELECT clock_timestamp()<expires_at FROM relay.outbox_events").fetch_one(&r.pool).await,"live retained lease");assert!(live);
            until(&r.pool,"SELECT status='processed' AND attempt_count=2 AND ownership_token IS NULL FROM relay.outbox_events").await;
            message(&r.channel,&r.topology.queue,id).await;empty(&r.channel,&r.topology.queue).await;stop_tx.send(()).unwrap();
        };
        tokio::join!(reliable_event_relay::application::worker::run(&repository,&lifecycle,&config,&mut shutdown),observer);
        assert_eq!(lifecycle.connections.load(std::sync::atomic::Ordering::Acquire),2);
        println!("real worker core/instrumented lifecycle: Uncertain, publisher replaced; live retained claim count1; no blind replay; expiry processed count2 with original duplicate ID");
    }).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn harness_cleans_exact_resources_after_assertion_failure() {
    let identities = Arc::new(Mutex::new(None));
    let captured = identities.clone();
    let harness = tokio::spawn(async move {
        experiment(move |r| async move {
            let id = Uuid::now_v7();
            seed(&r.pool, id).await;
            let database: String = checked(
                sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(&r.pool)
                    .await,
                "failure fixture identity",
            );
            r.spawn(None, None).await;
            r.log_until(0, "relay started").await;
            let pid = r.children.lock().unwrap()[0].id().unwrap();
            let (_, _, proxy) = faults::proxy().await;
            r.proxies.lock().unwrap().push(proxy);
            *captured.lock().unwrap() = Some((database, r.topology.clone(), r.temp.clone(), pid));
            panic!("intentional cleanup regression assertion");
        })
        .await;
    });
    assert!(harness.await.unwrap_err().is_panic());
    let (database, topology, temp, pid) = identities
        .lock()
        .unwrap()
        .take()
        .expect("captured owned resources");
    assert!(!temp.exists());
    assert!(
        !Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap()
            .success(),
        "owned child must be reaped"
    );
    let connection = Connection::connect(&faults::uri(), ConnectionProperties::default())
        .await
        .expect("cleanup observer");
    let channel = connection.create_channel().await.unwrap();
    assert!(
        channel
            .queue_declare(
                topology.queue.into(),
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                Default::default()
            )
            .await
            .is_err()
    );
    let channel = connection.create_channel().await.unwrap();
    assert!(
        channel
            .exchange_declare(
                topology.exchange.into(),
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                Default::default()
            )
            .await
            .is_err()
    );
    connection
        .close(200, "verification close".into())
        .await
        .unwrap();
    isolated(move |pool| async move {
        let remains: bool = checked(
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname=$1)")
                .bind(database)
                .fetch_one(&pool)
                .await,
            "failed fixture database absent",
        );
        assert!(!remains);
        Ok(())
    })
    .await;
    println!(
        "intentional assertion failure: exact child reaped, proxy joined, temp removed, broker resources and database independently absent"
    );
}
