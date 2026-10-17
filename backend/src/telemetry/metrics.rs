//! Fixed-size process registry and bounded loopback-only Prometheus text exporter.
use crate::application::observation::{Observer, Operation, Outcome};
use std::{
    fmt::Write,
    io,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
    time::timeout,
};

pub const BUCKETS: [f64; 8] = [0.001, 0.005, 0.025, 0.1, 0.5, 1.0, 5.0, 30.0];
#[derive(Clone, Copy)]
pub enum Process {
    Http,
    Relay,
    Consumer,
}
impl Process {
    pub fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Relay => "relay",
            Self::Consumer => "consumer",
        }
    }
}
fn add(counter: &AtomicU64, n: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(v.saturating_add(n))
    });
}
#[derive(Clone, Copy, Default)]
struct Timing {
    buckets: [u64; 8],
    count: u64,
    micros: u64,
}
/// No dynamic labels, registrations, payloads, database calls or remote writes.
pub struct Metrics {
    pub readiness: Arc<crate::application::readiness::Readiness>,
    process: Process,
    events: [[AtomicU64; 25]; 16],
    flight: [AtomicU64; 16],
    timings: [std::sync::Mutex<Timing>; 16],
    pool_max: AtomicU64,
    prefetch: AtomicU64,
    replay: AtomicU64,
    exporter_connections: AtomicU64,
}
impl Metrics {
    pub fn new(process: Process) -> Arc<Self> {
        Arc::new(Self {
            readiness: Arc::new(crate::application::readiness::Readiness::new(
                !matches!(process, Process::Http),
                !matches!(process, Process::Http),
            )),
            process,
            events: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
            flight: std::array::from_fn(|_| AtomicU64::new(0)),
            timings: std::array::from_fn(|_| std::sync::Mutex::new(Timing::default())),
            pool_max: AtomicU64::new(0),
            prefetch: AtomicU64::new(0),
            replay: AtomicU64::new(0),
            exporter_connections: AtomicU64::new(0),
        })
    }
    pub fn configure(&self, pool_max: u32, prefetch: u16, replay_enabled: bool) {
        self.pool_max.store(u64::from(pool_max), Ordering::Relaxed);
        self.prefetch.store(u64::from(prefetch), Ordering::Relaxed);
        self.replay
            .store(u64::from(replay_enabled), Ordering::Relaxed);
    }
    pub fn count(&self, operation: Operation, outcome: Outcome) -> u64 {
        self.events[operation as usize][outcome as usize].load(Ordering::Relaxed)
    }
    pub fn in_flight(&self, operation: Operation) -> u64 {
        self.flight[operation as usize].load(Ordering::Relaxed)
    }
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(32768);
        let process = self.process.label();
        writeln!(out, "# HELP relay_observations_total Process-local phase outcomes; not business totals.\n# TYPE relay_observations_total counter").unwrap();
        for op in Operation::ALL {
            for outcome in Outcome::ALL {
                let n = self.count(op, outcome);
                if n > 0 {
                    writeln!(out, "relay_observations_total{{process=\"{process}\",operation=\"{}\",outcome=\"{}\"}} {n}",op.label(), outcome.label()).unwrap();
                }
            }
        }
        writeln!(out, "# HELP relay_in_flight Active observed futures, not broker credit or database leases.\n# TYPE relay_in_flight gauge").unwrap();
        for op in Operation::ALL {
            writeln!(
                out,
                "relay_in_flight{{process=\"{process}\",operation=\"{}\"}} {}",
                op.label(),
                self.in_flight(op)
            )
            .unwrap();
        }
        writeln!(out, "# HELP relay_operation_duration_seconds Local elapsed phase time including cancellation.\n# TYPE relay_operation_duration_seconds histogram").unwrap();
        for op in Operation::ALL {
            // Copy under a short local lock, then format after releasing it.
            let t = *self.timings[op as usize]
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (i, b) in BUCKETS.iter().enumerate() {
                writeln!(out, "relay_operation_duration_seconds_bucket{{process=\"{process}\",operation=\"{}\",le=\"{b}\"}} {}",op.label(),t.buckets[i]).unwrap();
            }
            let n = t.count;
            writeln!(out, "relay_operation_duration_seconds_bucket{{process=\"{process}\",operation=\"{}\",le=\"+Inf\"}} {n}",op.label()).unwrap();
            writeln!(out, "relay_operation_duration_seconds_count{{process=\"{process}\",operation=\"{}\"}} {n}",op.label()).unwrap();
            writeln!(
                out,
                "relay_operation_duration_seconds_sum{{process=\"{process}\",operation=\"{}\"}} {}",
                op.label(),
                t.micros as f64 / 1_000_000.0
            )
            .unwrap();
        }
        for (name, help, value) in [
            (
                "relay_exporter_connections",
                "Active exporter connections, maximum four.",
                self.exporter_connections.load(Ordering::Relaxed),
            ),
            (
                "relay_pool_connections_limit",
                "Configured connection limit; zero means no pool configured.",
                self.pool_max.load(Ordering::Relaxed),
            ),
            (
                "relay_prefetch_limit",
                "Configured consumer credit; zero means process has no subscription.",
                self.prefetch.load(Ordering::Relaxed),
            ),
            (
                "relay_processing_limit",
                "Sequential delivery limit; HTTP has no delivery loop.",
                u64::from(!matches!(self.process, Process::Http)),
            ),
            (
                "relay_replay_admission_limit",
                "Maximum active replay requests; zero means disabled.",
                self.replay.load(Ordering::Relaxed) * 2,
            ),
        ] {
            writeln!(
                out,
                "# HELP {name} {help}\n# TYPE {name} gauge\n{name}{{process=\"{process}\"}} {value}"
            )
            .unwrap();
        }
        let snapshot = self.readiness.snapshot();
        writeln!(out, "# TYPE relay_readiness_pool_connections_limit gauge\nrelay_readiness_pool_connections_limit{{process=\"{process}\"}} {}", u8::from(snapshot.required[0])).unwrap();
        out.push_str("# TYPE relay_readiness_reason gauge\n# TYPE relay_dependency_observed gauge\n# TYPE relay_dependency_fresh gauge\n# TYPE relay_dependency_failures_total counter\n# TYPE relay_dependency_last_success_timestamp_seconds gauge\n# TYPE relay_dependency_observation_age_seconds gauge\n# TYPE relay_dependency_observation_duration_seconds gauge\n");
        for reason in crate::application::readiness::Reason::ALL {
            writeln!(
                out,
                "relay_readiness_reason{{process=\"{process}\",reason=\"{}\"}} {}",
                reason.label(),
                u8::from(snapshot.reason == reason)
            )
            .unwrap();
        }
        writeln!(
            out,
            "# TYPE relay_ready gauge\nrelay_ready{{process=\"{process}\"}} {}",
            u8::from(snapshot.ready())
        )
        .unwrap();
        for (i, dependency) in ["database", "broker"].into_iter().enumerate() {
            if !snapshot.required[i] {
                continue;
            }
            writeln!(
                out,
                "relay_dependency_observed{{process=\"{process}\",dependency=\"{dependency}\"}} {}",
                u8::from(snapshot.age[i].is_some())
            )
            .unwrap();
            writeln!(
                out,
                "relay_dependency_fresh{{process=\"{process}\",dependency=\"{dependency}\"}} {}",
                u8::from(
                    snapshot.age[i]
                        .is_some_and(|age| age <= crate::application::readiness::FRESHNESS)
                )
            )
            .unwrap();
            writeln!(out,"relay_dependency_failures_total{{process=\"{process}\",dependency=\"{dependency}\"}} {}",snapshot.failures[i]).unwrap();
            if let Some(at) = snapshot.last_success[i] {
                writeln!(out,"relay_dependency_last_success_timestamp_seconds{{process=\"{process}\",dependency=\"{dependency}\"}} {at}").unwrap();
            }
            if let Some(age) = snapshot.age[i] {
                writeln!(out,"relay_dependency_observation_age_seconds{{process=\"{process}\",dependency=\"{dependency}\"}} {}",age.as_secs_f64()).unwrap();
                writeln!(out,"relay_dependency_observation_duration_seconds{{process=\"{process}\",dependency=\"{dependency}\"}} {}",snapshot.duration[i].as_secs_f64()).unwrap();
            }
        }
        out
    }
}
impl Observer for Metrics {
    fn broker_readiness(&self, usable: bool) {
        self.readiness.observe(
            crate::application::readiness::Dependency::Broker,
            usable,
            Duration::ZERO,
        );
    }
    fn event(&self, operation: Operation, outcome: Outcome) {
        add(&self.events[operation as usize][outcome as usize], 1);
    }
    fn enter(&self, operation: Operation) {
        add(&self.flight[operation as usize], 1);
    }
    fn exit(&self, operation: Operation, elapsed: Duration) {
        self.flight[operation as usize].fetch_sub(1, Ordering::Relaxed);
        let mut t = self.timings[operation as usize]
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for (i, b) in BUCKETS.iter().enumerate() {
            if elapsed.as_secs_f64() <= *b {
                t.buckets[i] = t.buckets[i].saturating_add(1);
            }
        }
        t.micros = t
            .micros
            .saturating_add(u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX));
        t.count = t.count.saturating_add(1);
    }
}
/// All three addresses validated together, even in separate processes.
pub struct ExporterConfig {
    addresses: [SocketAddr; 3],
}
impl ExporterConfig {
    pub fn from_lookup(
        get: impl Fn(&str) -> Option<String>,
        http: SocketAddr,
    ) -> Result<Self, &'static str> {
        let mut addresses: [SocketAddr; 3] = [
            "127.0.0.1:9100".parse().unwrap(),
            "127.0.0.1:9101".parse().unwrap(),
            "127.0.0.1:9102".parse().unwrap(),
        ];
        for (i, key) in [
            "HTTP_METRICS_ADDR",
            "RELAY_METRICS_ADDR",
            "CONSUMER_METRICS_ADDR",
        ]
        .iter()
        .enumerate()
        {
            if let Some(raw) = get(key) {
                addresses[i] = raw.parse().map_err(|_| "invalid metrics address")?;
            }
            if !addresses[i].ip().is_loopback() || addresses[i].port() == 0 {
                return Err("metrics require loopback IP and nonzero port");
            }
            if addresses[i].port() == http.port()
                || addresses[..i]
                    .iter()
                    .any(|a| a.port() == addresses[i].port())
            {
                return Err("metrics/HTTP ports must be distinct");
            }
        }
        Ok(Self { addresses })
    }
    pub fn address(&self, process: Process) -> SocketAddr {
        self.addresses[match process {
            Process::Http => 0,
            Process::Relay => 1,
            Process::Consumer => 2,
        }]
    }
}
struct ConnectionObservation(Arc<Metrics>);
impl Drop for ConnectionObservation {
    fn drop(&mut self) {
        self.0.exporter_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
/// Owns one accept task and at most four connection tasks. Drop aborts the task tree.
pub struct Exporter {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Exporter {
    pub async fn start(address: SocketAddr, metrics: Arc<Metrics>) -> io::Result<Self> {
        if !address.ip().is_loopback() {
            return Err(io::Error::other("metrics require loopback"));
        }
        let listener = TcpListener::bind(address).await?;
        let address = listener.local_addr()?;
        let (tx, mut rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! { biased;
                    _=&mut rx=>break,
                    Some(_)=clients.join_next(), if !clients.is_empty()=>{},
                    accepted=listener.accept(), if clients.len()<4=>match accepted {
                        Ok((socket,_))=>{
                            let metrics=metrics.clone();add(&metrics.exporter_connections,1);
                            let guard=ConnectionObservation(metrics.clone());
                            clients.spawn(async move { let _guard=guard; let _=timeout(Duration::from_secs(2), serve(socket,metrics)).await; });
                        },
                        Err(_)=>{tracing::error!("metrics accept failed; exporter stopped");break;}
                    }
                }
            }
            clients.abort_all();
            while clients.join_next().await.is_some() {}
        });
        Ok(Self {
            address,
            stop: Some(tx),
            task,
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if timeout(Duration::from_secs(1), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            let _ = (&mut self.task).await;
        }
    }
}
impl Drop for Exporter {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn serve(mut socket: TcpStream, metrics: Arc<Metrics>) -> io::Result<()> {
    let mut buffer = [0u8; 1024];
    let mut used = 0;
    loop {
        if used == buffer.len() {
            return Ok(());
        }
        let n = socket.read(&mut buffer[used..]).await?;
        if n == 0 {
            return Ok(());
        }
        used += n;
        if buffer[..used].windows(4).any(|v| v == b"\r\n\r\n") {
            break;
        }
    }
    let request = buffer[..used]
        .split(|b| *b == b'\r')
        .next()
        .unwrap_or_default();
    let readiness = request == b"GET /ready HTTP/1.1" || request == b"GET /ready HTTP/1.0";
    let metrics_path = request == b"GET /metrics HTTP/1.1" || request == b"GET /metrics HTTP/1.0";
    let (status, content_type, body) = if readiness {
        let snapshot = metrics.readiness.snapshot();
        (if snapshot.ready() {"200 OK"} else {"503 Service Unavailable"}, "application/json",serde_json::json!({"process":metrics.process.label(),"state":snapshot.state(),"reason":snapshot.reason.label(),"freshness_seconds":crate::application::readiness::FRESHNESS.as_secs(),"observation_age_seconds":{"database":snapshot.age[0].map(|v|v.as_secs_f64()),"broker":snapshot.age[1].map(|v|v.as_secs_f64())},"last_success_timestamp_seconds":{"database":snapshot.last_success[0],"broker":snapshot.last_success[1]}}).to_string())
    } else if metrics_path {
        (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            metrics.render(),
        )
    } else {
        ("404 Not Found", "text/plain", String::new())
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(header.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await?;
    socket.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::observation::{Observation, observe};
    #[test]
    fn validates_all_addresses_and_collision_combinations() {
        let http = "0.0.0.0:8080".parse().unwrap();
        assert!(ExporterConfig::from_lookup(|_| None, http).is_ok());
        for value in [
            "0.0.0.0:9100",
            "127.0.0.1:0",
            "localhost:9100",
            "127.0.0.1:8080",
            "127.0.0.1:9101",
        ] {
            assert!(
                ExporterConfig::from_lookup(
                    |key| (key == "HTTP_METRICS_ADDR").then(|| value.into()),
                    http
                )
                .is_err()
            );
        }
    }
    #[tokio::test]
    async fn fixed_registry_tracks_cancellation_and_cumulative_buckets() {
        let metrics = Metrics::new(Process::Consumer);
        let mut future = Box::pin(observe(
            metrics.as_ref(),
            Operation::Processing,
            std::future::pending::<()>(),
            |_| Outcome::Applied,
        ));
        assert!(futures_util::poll!(&mut future).is_pending());
        assert_eq!(metrics.in_flight(Operation::Processing), 1);
        drop(future);
        assert_eq!(metrics.in_flight(Operation::Processing), 0);
        assert_eq!(metrics.count(Operation::Processing, Outcome::Cancelled), 1);
        assert_eq!(metrics.count(Operation::Processing, Outcome::Applied), 0);
        let guard = Observation::new(metrics.as_ref(), Operation::Ack);
        guard.finish(Outcome::Completed);
        let out = metrics.render();
        assert!(out.contains("# TYPE relay_operation_duration_seconds histogram"));
        assert!(out.contains("operation=\"ack_send\",le=\"+Inf\"} 1"));
        // Maximum reachable fixed enum cardinality cannot grow with user data.
        for op in Operation::ALL {
            for outcome in Outcome::ALL {
                metrics.event(op, outcome);
            }
        }
        let out = metrics.render();
        assert!(out.len() < 100_000);
        assert_eq!(
            out.lines()
                .filter(|l| l.starts_with("relay_observations_total{"))
                .count(),
            16 * 25
        );
        assert!(!out.contains("event_id"));
        assert!(!out.contains("sku"));
    }
    #[tokio::test]
    async fn exporter_admits_at_most_four_connections_and_cleans_up_stalled_clients() {
        let metrics = Metrics::new(Process::Http);
        let exporter = Exporter::start("127.0.0.1:0".parse().unwrap(), metrics.clone())
            .await
            .unwrap();
        let mut clients = Vec::new();
        for _ in 0..4 {
            clients.push(TcpStream::connect(exporter.address()).await.unwrap());
        }
        timeout(Duration::from_secs(1), async {
            while metrics.exporter_connections.load(Ordering::Relaxed) != 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut fifth = TcpStream::connect(exporter.address()).await.unwrap();
        fifth
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        assert_eq!(metrics.exporter_connections.load(Ordering::Relaxed), 4);
        drop(clients.remove(0));
        let mut response = String::new();
        timeout(Duration::from_secs(1), fifth.read_to_string(&mut response))
            .await
            .unwrap()
            .unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        exporter.shutdown().await;
        assert_eq!(metrics.exporter_connections.load(Ordering::Relaxed), 0);
    }
    #[tokio::test]
    async fn exporter_serves_only_metrics_and_releases_listener_on_shutdown() {
        let metrics = Metrics::new(Process::Relay);
        let exporter = Exporter::start("127.0.0.1:0".parse().unwrap(), metrics)
            .await
            .unwrap();
        let address = exporter.address();
        assert!(
            Exporter::start(address, Metrics::new(Process::Http))
                .await
                .is_err()
        );
        let mut socket = TcpStream::connect(address).await.unwrap();
        socket
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut out = String::new();
        socket.read_to_string(&mut out).await.unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK"));
        assert!(out.contains("process=\"relay\""));
        let _stalled = TcpStream::connect(address).await.unwrap();
        exporter.shutdown().await;
        let rebound = TcpListener::bind(address).await.unwrap();
        drop(rebound);
    }
}
