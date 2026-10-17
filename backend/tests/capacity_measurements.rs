//! Finite, opt-in measurements on dedicated disposable services only.
//! See docs/en/operations/milestone-5-3.md for population and timing definitions.
mod support;
use lapin::{Connection, ConnectionProperties, options::*, types::FieldTable};
use reliable_event_relay::{
    application::publisher::EventPublisher,
    domain::event::EventEnvelope,
    infrastructure::rabbitmq::{RabbitMqConfig, RabbitMqPublisher, Topology},
};
use sqlx::{Connection as _, PgPool};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use support::{TestResult, checked};
use tokio::{
    process::{Child, Command},
    time::{sleep, timeout},
};
use uuid::Uuid;

struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

const N: usize = 80;
const POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Baseline,
    Duplicates,
    Contended,
    Larger,
    Paused,
    Locked,
    Invalid,
    Stale,
}

#[derive(Default)]
struct Samples {
    inserted: HashMap<Uuid, Instant>,
    seen: HashMap<Uuid, f64>,
    peak_rss_kib: [u64; 3],
    cpu_ticks_first: [Option<u64>; 3],
    cpu_ticks_last: [Option<u64>; 3],
    max_connections: i64,
    max_lock_waiters: i64,
    max_ready: u32,
    polls: usize,
}
fn proc_sample(pid: u32) -> Option<(u64, u64)> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss = status
        .lines()
        .find(|s| s.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    Some((
        rss,
        fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?,
    ))
}
async fn signal(child: &Child, signal: &str) {
    assert!(
        Command::new("kill")
            .args([signal, &child.id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
}
async fn stop(child: &mut Child) {
    signal(child, "-CONT").await;
    signal(child, "-TERM").await;
    assert!(
        timeout(Duration::from_secs(28), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
fn event(i: usize, scenario: Scenario) -> EventEnvelope {
    let large = matches!(scenario, Scenario::Larger);
    let sku = if large {
        format!("S{:0>126}", i)
    } else if matches!(scenario, Scenario::Contended | Scenario::Stale) {
        "same".into()
    } else {
        format!("S{i}")
    };
    let envelope = EventEnvelope::new(
        if matches!(scenario, Scenario::Invalid) { "unsupported" } else { "inventory.units_received" },
        if large { "a".repeat(128) } else { "stock".into() },
        if large { "b".repeat(128) } else { "1".into() },
        1,
        serde_json::json!({"warehouse_id":if large {"w".repeat(128)} else {"local".into()},"sku":sku,"quantity":1}),
    ).unwrap();
    // Match PostgreSQL's microsecond occurrence representation before duplicating.
    let mut value = serde_json::to_value(&envelope).unwrap();
    value["occurred_at"] = serde_json::json!(
        envelope
            .occurred_at()
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
    );
    serde_json::from_value(value).unwrap()
}

async fn insert(pool: &PgPool, e: &EventEnvelope) {
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(e.id()).bind(e.event_type().as_str()).bind(e.aggregate_type()).bind(e.aggregate_id())
        .bind(i32::try_from(e.schema_version()).unwrap()).bind(e.occurred_at()).bind(e.payload())
        .execute(pool).await, "measurement insert");
}
async fn counts(pool: &PgPool) -> (i64, i64, i64) {
    checked(sqlx::query_as("SELECT (SELECT count(*) FROM relay.outbox_events WHERE status='processed'), (SELECT count(*) FROM consumer.receipts), (SELECT COALESCE(sum(units),0)::bigint FROM consumer.stock)").fetch_one(pool).await, "reconcile")
}
async fn drain(pool: &PgPool, expected: i64, receipts: i64) {
    timeout(Duration::from_secs(12), async {
        loop {
            let c = counts(pool).await;
            if c.0 == expected && c.1 == receipts {
                break;
            }
            sleep(POLL).await;
        }
    })
    .await
    .expect("finite drain deadline");
}
fn metric(text: &str, op: &str, outcome: &str) -> u64 {
    text.lines()
        .filter(|l| {
            l.starts_with("relay_observations_total{")
                && l.contains(&format!("operation=\"{op}\""))
                && l.contains(&format!("outcome=\"{outcome}\""))
        })
        .filter_map(|l| l.split_whitespace().last()?.parse::<u64>().ok())
        .sum()
}
async fn ready_response(address: std::net::SocketAddr) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    timeout(Duration::from_secs(3), async {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket
            .write_all(b"GET /ready HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut text = String::new();
        socket.read_to_string(&mut text).await.unwrap();
        text
    })
    .await
    .unwrap()
}
async fn consumer_metrics(addresses: &[[std::net::SocketAddr; 3]]) -> String {
    let mut text = String::new();
    for ports in &addresses[1..] {
        text.push_str(&support::scrape(ports[2]).await);
    }
    text
}
async fn run(scenario: Scenario, repetition: usize) {
    assert_eq!(
        std::env::var("RELAY_DEDICATED_MEASUREMENTS").as_deref(),
        Ok("1"),
        "explicit dedicated service authorization required"
    );
    support::isolated(move |pool| async move {
        for migration in [
            include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
            include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),
            include_str!("../migrations/20261013000000_add_audited_replay.up.sql"),
            include_str!("../migrations/20261014000000_add_atomic_consumer.up.sql"),
        ] { checked(sqlx::raw_sql(migration).execute(&pool).await,"measurement schema"); }
        let prefix=format!("relay.it.measure.{}",Uuid::now_v7().simple());
        let topology=|suffix:&str| Topology { exchange:format!("{prefix}.{suffix}.exchange"),queue:format!("{prefix}.{suffix}.queue"),routing_key:"event".into() };
        let source=topology("source"); let quarantine=topology("quarantine");
        let mut uri=url::Url::parse("amqp://rabbitmq/relay").unwrap();
        uri.set_username(&std::env::var("RABBITMQ_DEFAULT_USER").unwrap()).unwrap();
        uri.set_password(Some(&std::env::var("RABBITMQ_DEFAULT_PASS").unwrap())).unwrap();
        uri.set_host(Some(&std::env::var("RABBITMQ_HOST").unwrap())).unwrap();
        uri.set_port(Some(std::env::var("RABBITMQ_PORT").unwrap_or_else(|_|"5672".into()).parse().unwrap())).unwrap();
        uri.path_segments_mut().unwrap().clear().push(&std::env::var("RABBITMQ_DEFAULT_VHOST").unwrap());
        let connection=timeout(Duration::from_secs(8),Connection::connect(uri.as_str(),ConnectionProperties::default())).await.unwrap().unwrap();
        let channel=connection.create_channel().await.unwrap();
        let publisher=RabbitMqPublisher::connect(RabbitMqConfig::new(uri.to_string(),source.clone(),Duration::from_secs(3),Duration::from_secs(3)).unwrap()).await.unwrap();
        let database:String=checked(sqlx::query_scalar("SELECT current_database()").fetch_one(&pool).await,"measurement database");
        let mut children:Vec<Child>=Vec::new(); let mut addresses=Vec::new();
        let mut binaries=vec![env!("CARGO_BIN_EXE_relay"),env!("CARGO_BIN_EXE_consumer")];
        if matches!(scenario,Scenario::Contended) {binaries.push(env!("CARGO_BIN_EXE_consumer"));}
        for binary in binaries {
            let mut command=Command::new(binary); let ports=support::metrics_env(&mut command);
            command.env("POSTGRES_DB",&database).env("REPLAY_OPERATORS","")
                .env("RABBITMQ_EXCHANGE",&source.exchange).env("RABBITMQ_QUEUE",&source.queue).env("RABBITMQ_ROUTING_KEY","event")
                .env("CONSUMER_SOURCE_EXCHANGE",&source.exchange).env("CONSUMER_SOURCE_QUEUE",&source.queue).env("CONSUMER_SOURCE_ROUTING_KEY","event")
                .env("CONSUMER_QUARANTINE_EXCHANGE",&quarantine.exchange).env("CONSUMER_QUARANTINE_QUEUE",&quarantine.queue).env("CONSUMER_QUARANTINE_ROUTING_KEY","event")
                .env("CONSUMER_PREFETCH","1").env("HTTP_ADDR","127.0.0.1:0")
                .stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
            if matches!(scenario,Scenario::Stale) {command.env("CONSUMER_STATEMENT_MS","30000").env("CONSUMER_LOCK_MS","25000").env("CONSUMER_OPERATION_MS","30000");}
            children.push(command.spawn().unwrap()); addresses.push(ports);
        }
        // Cleanup outside the caught task also runs after failed assertions.
        let p=pool.clone(); let ch=channel.clone(); let src=source.clone(); let q=quarantine.clone();
        let case=async {
            support::wait_ready(addresses[0][1],"relay").await;
            for ports in &addresses[1..] {support::wait_ready(ports[2],"consumer").await;}
            for i in 1000..1008 {insert(&p,&event(i,Scenario::Baseline)).await;}
            drain(&p,8,8).await;
            // ACK observations follow receipt COMMIT; wait separately before warm-up snapshot.
            timeout(Duration::from_secs(4),async {loop {if metric(&consumer_metrics(&addresses).await,"ack_send","completed")==8 {break;} sleep(POLL).await;}}).await.unwrap();
            let before_relay=support::scrape(addresses[0][1]).await;
            let before_consumer=consumer_metrics(&addresses).await;
            let samples=Arc::new(Mutex::new(Samples::default()));
            let events:Vec<_>=(0..N).map(|i|event(i,scenario)).collect();
            let payload_bytes:Vec<_>=events.iter().map(|e|serde_json::to_vec(e).unwrap().len()).collect();
            let pids:Vec<_>=children.iter().map(|c|c.id().unwrap()).collect();
            let observer_pool=p.clone(); let observer_samples=samples.clone(); let observer_ch=ch.clone(); let queue=src.queue.clone();
            let mut observer=tokio::spawn(async move {
                for _ in 0..1500 {
                    let ids:Vec<Uuid>=checked(sqlx::query_scalar("SELECT event_id FROM consumer.receipts").fetch_all(&observer_pool).await,"bounded receipt observer");
                    let observed=Instant::now();
                    let resources:(i64,i64)=checked(sqlx::query_as("SELECT count(*),count(*) FILTER (WHERE wait_event_type='Lock') FROM pg_stat_activity WHERE datname=current_database()").fetch_one(&observer_pool).await,"resource observer");
                    let ready=observer_ch.queue_declare(queue.clone().into(),QueueDeclareOptions{passive:true,..Default::default()},FieldTable::default()).await.unwrap().message_count();
                    {let mut s=observer_samples.lock().unwrap(); s.polls+=1;
                    for id in ids {if let Some(t)=s.inserted.get(&id) {let elapsed=observed.duration_since(*t).as_secs_f64(); s.seen.entry(id).or_insert(elapsed);}}
                    for (i,pid) in pids.iter().enumerate() {if let Some((rss,ticks))=proc_sample(*pid) {s.peak_rss_kib[i]=s.peak_rss_kib[i].max(rss); s.cpu_ticks_first[i].get_or_insert(ticks); s.cpu_ticks_last[i]=Some(ticks);}}
                    s.max_connections=s.max_connections.max(resources.0); s.max_lock_waiters=s.max_lock_waiters.max(resources.1); s.max_ready=s.max_ready.max(ready);}
                    sleep(POLL).await;
                }
            });
            let _observer_guard=AbortOnDrop(observer.abort_handle());
            if matches!(scenario,Scenario::Locked) {signal(&children[0],"-STOP").await;}
            if matches!(scenario,Scenario::Paused) {signal(&children[1],"-STOP").await;}
            let mut stock_barrier=if matches!(scenario,Scenario::Contended|Scenario::Stale) {
                sqlx::query("INSERT INTO consumer.stock (warehouse_id,sku,units) VALUES ('local','same',0)").execute(&p).await.unwrap();
                let mut connection=sqlx::PgConnection::connect_with(&p.connect_options()).await.unwrap();
                sqlx::query("BEGIN").execute(&mut connection).await.unwrap();
                sqlx::query("SELECT units FROM consumer.stock WHERE warehouse_id='local' AND sku='same' FOR UPDATE").execute(&mut connection).await.unwrap();
                Some(connection)
            } else {None};
            let begin=Instant::now(); let interval=if matches!(scenario,Scenario::Paused|Scenario::Locked) {Duration::from_millis(2)} else {Duration::from_millis(20)};
            let mut generator_late=0; let mut insert_seconds=0.0;
            // Completion-paced arrivals, no catch-up queue or spawned per-event work.
            for e in &events {let start=Instant::now();samples.lock().unwrap().inserted.insert(e.id(),start); insert(&p,e).await;let elapsed=start.elapsed();insert_seconds+=elapsed.as_secs_f64();if elapsed>interval {generator_late+=1;}sleep(interval.saturating_sub(elapsed)).await;}
            let admission_elapsed=begin.elapsed().as_secs_f64();
            if let Some(mut barrier)=stock_barrier.take() {
                if matches!(scenario,Scenario::Stale) {
                    sleep(Duration::from_secs(17)).await;
                    let response=ready_response(addresses[1][2]).await;
                    assert!(response.starts_with("HTTP/1.1 503"));assert!(response.contains("broker_stale"));
                } else {assert!(samples.lock().unwrap().max_lock_waiters>=2,"two actual concurrent stock waiters required");}
                sqlx::query("ROLLBACK").execute(&mut barrier).await.unwrap();barrier.close().await.unwrap();
            }
            let mut backlog_at_release=None;
            if matches!(scenario,Scenario::Locked) {
                // Lock only pending rows after admission; acquisitions use SKIP LOCKED.
                let mut tx=p.begin().await.unwrap();
                let held:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM relay.outbox_events WHERE status='pending' FOR UPDATE").fetch_all(&mut *tx).await.unwrap();
                assert_eq!(held.len(),N); signal(&children[0],"-CONT").await; sleep(Duration::from_millis(750)).await;backlog_at_release=Some(held.len());tx.rollback().await.unwrap();
            }
            if matches!(scenario,Scenario::Paused) {
                sleep(Duration::from_millis(750)).await;
                backlog_at_release=Some(ch.queue_declare(src.queue.clone().into(),QueueDeclareOptions{passive:true,..Default::default()},FieldTable::default()).await.unwrap().message_count() as usize);
                signal(&children[1],"-CONT").await;
            }
            let release=Instant::now();
            let expected_receipts=if matches!(scenario,Scenario::Invalid) {8} else {8+N as i64};
            drain(&p,8+N as i64,expected_receipts).await;
            if matches!(scenario,Scenario::Duplicates) {for e in &events {publisher.publish(e).await.unwrap();}}
            let expected_ack=8+N as u64+if matches!(scenario,Scenario::Duplicates) {N as u64} else {0};
            timeout(Duration::from_secs(8),async {loop {if metric(&consumer_metrics(&addresses).await,"ack_send","completed")==expected_ack {break;}sleep(POLL).await;}}).await.unwrap();
            for ports in &addresses[1..] {support::wait_ready(ports[2],"consumer").await;}
            let drain_seconds=release.elapsed().as_secs_f64();let total_seconds=begin.elapsed().as_secs_f64();
            // Let the final receipt observer complete; this is outside measured drain.
            if !matches!(scenario,Scenario::Invalid) {timeout(Duration::from_secs(2),async {loop {if samples.lock().unwrap().seen.len()==N {break;}sleep(POLL).await;}}).await.unwrap();}
            observer.abort();let _=(&mut observer).await;
            let after_relay=support::scrape(addresses[0][1]).await;let after_consumer=consumer_metrics(&addresses).await;
            let delta=|op,outcome,consumer| {let (after,before)=if consumer {(&after_consumer,&before_consumer)} else {(&after_relay,&before_relay)};metric(after,op,outcome)-metric(before,op,outcome)};
            assert_eq!(delta("acquisition","acquired",false),N as u64);
            assert_eq!(delta("publication","confirmed",false),N as u64);
            assert_eq!(delta("completion","applied",false),N as u64);
            let valid=!matches!(scenario,Scenario::Invalid);
            assert_eq!(delta("processing","applied",true),if valid {N as u64} else {0});
            assert_eq!(delta("processing","duplicate",true),if matches!(scenario,Scenario::Duplicates) {N as u64} else {0});
            assert_eq!(delta("ack_send","completed",true),expected_ack-8);
            assert_eq!(delta("quarantine","confirmed",true),if valid {0} else {N as u64});
            assert_eq!(counts(&p).await,(8+N as i64,expected_receipts,expected_receipts));
            let dead:i64=sqlx::query_scalar("SELECT count(*) FROM relay.outbox_events WHERE status='dead_letter'").fetch_one(&p).await.unwrap();assert_eq!(dead,0);
            let mut quarantined=0; let mut quarantine_ids=std::collections::HashSet::new();
            while let Some(message)=ch.basic_get(q.queue.clone().into(),BasicGetOptions::default()).await.unwrap() {assert!(!valid);assert!(message.data.len()<16384);
                let original:EventEnvelope=serde_json::from_slice(&message.data).unwrap();
                assert!(events.contains(&original));assert!(quarantine_ids.insert(original.id()));
                assert_eq!(message.properties.message_id().as_ref().unwrap().as_str(),original.id().to_string());
                quarantined+=1;message.ack(BasicAckOptions::default()).await.unwrap();}
            assert_eq!(quarantined,if valid {0} else {N});
            assert_eq!(ch.queue_declare(src.queue.clone().into(),QueueDeclareOptions{passive:true,..Default::default()},FieldTable::default()).await.unwrap().message_count(),0);
            let s=samples.lock().unwrap(); assert!(s.cpu_ticks_first.iter().take(children.len()).all(Option::is_some)); let mut latency:Vec<_>=s.seen.values().copied().collect();latency.sort_by(f64::total_cmp);
            let percentile=|pct:usize|if latency.is_empty(){None}else{Some(latency[(latency.len()*pct).div_ceil(100)-1])};
            println!("MEASUREMENT {}",serde_json::json!({"scenario":format!("{scenario:?}"),"repetition":repetition,"unique":N,"consumer_processes":children.len()-1,"payload_bytes_min":payload_bytes.iter().min(),"payload_bytes_max":payload_bytes.iter().max(),"admission_seconds":admission_elapsed,"total_seconds":total_seconds,"offered_interval_ms":interval.as_millis(),"generator_late":generator_late,"insert_seconds_sum":insert_seconds,"drain_seconds":drain_seconds,"ready_at_release":backlog_at_release,"receipt_population":latency.len(),"receipt_observation_p50_seconds":percentile(50),"receipt_observation_p95_seconds":percentile(95),"peak_rss_kib":&s.peak_rss_kib[..children.len()],"cpu_ticks_start":&s.cpu_ticks_first[..children.len()],"cpu_ticks_end":&s.cpu_ticks_last[..children.len()],"max_database_connections":s.max_connections,"max_lock_waiters":s.max_lock_waiters,"sampled_max_broker_ready":s.max_ready,"observer_polls":s.polls,"quarantine_records":quarantined,"acquired":delta("acquisition","acquired",false),"publication_confirmed":delta("publication","confirmed",false),"completion_applied":delta("completion","applied",false),"consumer_applied":delta("processing","applied",true),"consumer_duplicate":delta("processing","duplicate",true),"ack_send_completed":delta("ack_send","completed",true),"unexplained":0}));
            Ok::<(),Box<dyn std::error::Error+Send+Sync>>(())
        };
        let outcome=std::panic::AssertUnwindSafe(case);
        use futures_util::FutureExt;
        let result=timeout(Duration::from_secs(35),outcome.catch_unwind()).await;
        for child in &mut children {if child.try_wait().unwrap().is_none(){stop(child).await;}}
        publisher.close().await.unwrap();
        for t in [&source,&quarantine] {
            channel.queue_delete(t.queue.clone().into(),QueueDeleteOptions::default()).await.unwrap();
            channel.exchange_delete(t.exchange.clone().into(),ExchangeDeleteOptions::default()).await.unwrap();
        }
        connection.close(200,"measurement cleanup".into()).await.unwrap();
        match result {Ok(Ok(r))=>r,Ok(Err(panic))=>std::panic::resume_unwind(panic),Err(_)=>panic!("measurement exceeded finite budget")}
    }).await;
}

#[tokio::test]
#[ignore = "dedicated disposable PostgreSQL/RabbitMQ only; explicit protocol required"]
async fn finite_capacity_and_recovery_campaign() -> TestResult {
    for scenario in [
        Scenario::Baseline,
        Scenario::Duplicates,
        Scenario::Contended,
        Scenario::Larger,
        Scenario::Paused,
        Scenario::Locked,
        Scenario::Invalid,
    ] {
        for repetition in 1..=3 {
            run(scenario, repetition).await;
        }
    }
    run(Scenario::Stale, 1).await;
    Ok(())
}
