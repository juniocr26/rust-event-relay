use reliable_event_relay::{
    application::{
        delivery::{DeliveryClaim, DeliveryOutcome, ReleaseOutcome},
        publisher::{EventPublisher, PublishError, PublishErrorKind},
        worker::{self, Decision, PublisherLifecycle},
    },
    config::WorkerConfig,
    domain::{
        delivery::{DeliveryLease, OwnershipToken},
        event::EventEnvelope,
    },
    persistence::*,
};
use std::{
    collections::VecDeque,
    future::pending,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::Notify,
    time::{Instant, advance},
};

struct Repo {
    tokens: Mutex<Vec<OwnershipToken>>,
    responses: Mutex<VecDeque<Result<Option<OwnedOutboxEvent>, PersistenceError>>>,
    admitted: Notify,
    block: AtomicBool,
}
impl Repo {
    fn new() -> Self {
        Self {
            tokens: Mutex::new(vec![]),
            responses: Mutex::new(VecDeque::new()),
            admitted: Notify::new(),
            block: AtomicBool::new(false),
        }
    }
}
impl OutboxAcquirer for Repo {
    async fn acquire(
        &self,
        request: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        self.tokens.lock().unwrap().push(request.token());
        self.admitted.notify_one();
        if self.block.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(None))
    }
}
impl OutboxCompleter for Repo {
    async fn complete(&self, _: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        Ok(TransitionOutcome::Applied)
    }
}
impl OutboxReleaser for Repo {
    async fn release(&self, _: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        Ok(TransitionOutcome::Applied)
    }
}
struct Publisher {
    sends: Arc<AtomicUsize>,
    error: Option<PublishErrorKind>,
}
impl EventPublisher for Publisher {
    async fn publish(&self, _: &EventEnvelope) -> Result<(), PublishError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        self.error
            .map_or(Ok(()), |kind| Err(PublishError::new(kind)))
    }
}
struct Lifecycle {
    sends: Arc<AtomicUsize>,
    connects: AtomicUsize,
    closes: AtomicUsize,
    fail: AtomicBool,
    connected: Notify,
    stuck_close: bool,
    usable: bool,
    errors: Mutex<VecDeque<Option<PublishErrorKind>>>,
}
impl Lifecycle {
    fn new() -> Self {
        Self {
            sends: Arc::new(AtomicUsize::new(0)),
            connects: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            connected: Notify::new(),
            stuck_close: false,
            usable: true,
            errors: Mutex::new(VecDeque::new()),
        }
    }
}
impl PublisherLifecycle for Lifecycle {
    type Publisher = Publisher;
    async fn connect(&self) -> Result<Publisher, PublishError> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        self.connected.notify_one();
        if self.fail.load(Ordering::SeqCst) {
            Err(PublishError::new(PublishErrorKind::Unavailable))
        } else {
            Ok(Publisher {
                sends: self.sends.clone(),
                error: self.errors.lock().unwrap().pop_front().flatten(),
            })
        }
    }
    fn usable(&self, _: &Publisher) -> bool {
        self.usable
    }
    async fn close(&self, _: Publisher) -> Result<(), PublishError> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        if self.stuck_close {
            pending::<()>().await;
        }
        Ok(())
    }
}
fn config() -> WorkerConfig {
    WorkerConfig::from_lookup(|_| None).unwrap()
}
fn claim() -> DeliveryClaim {
    let c = config();
    let token = OwnershipToken::fresh();
    DeliveryClaim {
        key: OwnedEventKey::new(uuid::Uuid::now_v7(), token),
        lease: DeliveryLease::new(
            token,
            chrono::DateTime::from_timestamp(0, 0).unwrap(),
            c.lease,
        )
        .unwrap(),
        attempt_count: 1,
    }
}
#[test]
fn exhaustive_outcome_policy_and_sanitized_diagnostics() {
    assert_eq!(worker::decision(&DeliveryOutcome::NoEvent), Decision::Idle);
    for kind in [
        PersistenceErrorKind::Unavailable,
        PersistenceErrorKind::InvalidStoredData,
        PersistenceErrorKind::OperationFailed,
        PersistenceErrorKind::CommitUncertain,
    ] {
        let error =
            PersistenceError::with_source(kind, std::io::Error::other("secret payload password"));
        let outcome = DeliveryOutcome::AcquisitionFailed {
            token: OwnershipToken::fresh(),
            duration: config().lease,
            error,
        };
        assert_eq!(worker::decision(&outcome), Decision::Pace);
        assert!(!format!("{outcome:?}").contains("secret"));
        assert_eq!(
            worker::decision(&DeliveryOutcome::Published {
                claim: claim(),
                completion: Err(PersistenceError::new(kind))
            }),
            Decision::Pace
        );
    }
    for transition in [TransitionOutcome::Applied, TransitionOutcome::OwnershipLost] {
        assert_eq!(
            worker::decision(&DeliveryOutcome::Published {
                claim: claim(),
                completion: Ok(transition)
            }),
            if transition == TransitionOutcome::Applied {
                Decision::Continue
            } else {
                Decision::Pace
            }
        );
    }
    for kind in [
        PublishErrorKind::Unavailable,
        PublishErrorKind::Serialization,
        PublishErrorKind::Rejected,
        PublishErrorKind::Unroutable,
        PublishErrorKind::Uncertain,
    ] {
        let expected = if matches!(
            kind,
            PublishErrorKind::Unavailable | PublishErrorKind::Uncertain
        ) {
            Decision::Replace
        } else {
            Decision::Pace
        };
        let releases = [
            ReleaseOutcome::LeaseRetained,
            ReleaseOutcome::Attempted(Ok(TransitionOutcome::Applied)),
            ReleaseOutcome::Attempted(Ok(TransitionOutcome::OwnershipLost)),
            ReleaseOutcome::Attempted(Err(PersistenceError::new(
                PersistenceErrorKind::OperationFailed,
            ))),
            ReleaseOutcome::Attempted(Err(PersistenceError::new(
                PersistenceErrorKind::CommitUncertain,
            ))),
        ];
        for release in releases {
            assert_eq!(
                worker::decision(&DeliveryOutcome::PublicationFailed {
                    claim: claim(),
                    error: PublishError::new(kind),
                    release
                }),
                expected
            );
        }
    }
}
#[test]
fn config_bounds_and_relations() {
    for key in [
        "RELAY_LEASE_MS",
        "RELAY_IDLE_MS",
        "RELAY_FAILURE_MS",
        "RELAY_RECONNECT_MS",
        "RELAY_POOL_MAX",
        "RELAY_POOL_ACQUIRE_MS",
        "RELAY_STATEMENT_MS",
        "RELAY_LOCK_MS",
        "RELAY_DRAIN_MS",
        "RELAY_SHUTDOWN_MS",
    ] {
        for bad in ["0", "-1", "18446744073709551615", "secret"] {
            assert!(WorkerConfig::from_lookup(|k| (k == key).then(|| bad.into())).is_err());
        }
    }
    assert!(
        WorkerConfig::from_lookup(|k| (k == "RELAY_SHUTDOWN_MS").then(|| "15000".into())).is_err()
    );
    assert_eq!(config().lease.as_micros(), 60_000_000);
}
#[tokio::test(start_paused = true)]
async fn idle_failure_pacing_fresh_tokens_and_interruptible_stop() {
    let repo = Repo::new();
    repo.responses.lock().unwrap().extend([
        Ok(None),
        Err(PersistenceError::new(PersistenceErrorKind::CommitUncertain)),
    ]);
    let lifecycle = Lifecycle::new();
    let stop = Notify::new();
    let cfg = config();
    let mut signal = Box::pin(stop.notified());
    let task = worker::run(&repo, &lifecycle, &cfg, &mut signal);
    tokio::pin!(task);
    tokio::select! { _ = &mut task => panic!("unexpected stop"), () = repo.admitted.notified() => {} }
    assert_eq!(repo.tokens.lock().unwrap().len(), 1);
    advance(cfg.idle - Duration::from_millis(1)).await;
    tokio::select! { _ = &mut task => panic!("stop"), _ = std::future::ready(()) => {} }
    assert_eq!(repo.tokens.lock().unwrap().len(), 1);
    advance(Duration::from_millis(1)).await;
    tokio::select! { _ = &mut task => panic!("stop"), () = repo.admitted.notified() => {} }
    let tokens = repo.tokens.lock().unwrap().clone();
    assert_eq!(tokens.len(), 2);
    assert_ne!(tokens[0], tokens[1]);
    stop.notify_one();
    task.await;
    assert_eq!(repo.tokens.lock().unwrap().len(), 2);
    assert_eq!(lifecycle.closes.load(Ordering::SeqCst), 1);
}
#[tokio::test(start_paused = true)]
async fn no_acquisition_without_publisher_and_reconnect_wait_interrupts() {
    let repo = Repo::new();
    let lifecycle = Lifecycle::new();
    lifecycle.fail.store(true, Ordering::SeqCst);
    let stop = Notify::new();
    let cfg = config();
    let mut signal = Box::pin(stop.notified());
    let task = worker::run(&repo, &lifecycle, &cfg, &mut signal);
    tokio::pin!(task);
    tokio::select! { _=&mut task=>panic!("stop"), ()=lifecycle.connected.notified()=>{} }
    assert!(repo.tokens.lock().unwrap().is_empty());
    assert_eq!(lifecycle.connects.load(Ordering::SeqCst), 1);
    stop.notify_one();
    task.await;
    assert_eq!(lifecycle.connects.load(Ordering::SeqCst), 1);
}
#[tokio::test(start_paused = true)]
async fn bounded_cancellation_and_cleanup_share_absolute_budget() {
    let repo = Repo::new();
    repo.block.store(true, Ordering::SeqCst);
    let mut lifecycle = Lifecycle::new();
    lifecycle.stuck_close = true;
    let stop = Notify::new();
    let cfg = config();
    let mut signal = Box::pin(stop.notified());
    let task = worker::run(&repo, &lifecycle, &cfg, &mut signal);
    tokio::pin!(task);
    tokio::select! { _=&mut task=>panic!("stop"), ()=repo.admitted.notified()=>{} }
    let start = Instant::now();
    stop.notify_one();
    let deadline = task.await;
    assert_eq!(deadline, start + cfg.shutdown);
    assert_eq!(Instant::now(), deadline);
    assert_eq!(repo.tokens.lock().unwrap().len(), 1);
}
#[tokio::test(start_paused = true)]
async fn in_flight_operation_drains_and_preserves_outcome() {
    let stop = Notify::new();
    stop.notify_one();
    let cfg = config();
    let start = Instant::now();
    let (result, deadline) = worker::drain_on_shutdown(
        async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            42
        },
        stop.notified(),
        cfg.drain,
        cfg.shutdown,
    )
    .await;
    assert_eq!(result, Some(42));
    assert_eq!(deadline, Some(start + cfg.shutdown));
}

#[tokio::test(start_paused = true)]
async fn publication_replaces_only_warranted_channels_without_replaying_claim() {
    for kind in [
        PublishErrorKind::Unavailable,
        PublishErrorKind::Uncertain,
        PublishErrorKind::Serialization,
        PublishErrorKind::Unroutable,
        PublishErrorKind::Rejected,
    ] {
        let repo = Repo::new();
        let c = claim();
        let event = EventEnvelope::new("test", "test", "1", 1, serde_json::Value::Null).unwrap();
        let snapshot = PendingOutboxEvent::new(event, 1, c.lease.acquired_at());
        let owned = OwnedOutboxEvent::new(snapshot, c.lease).unwrap();
        repo.responses
            .lock()
            .unwrap()
            .extend([Ok(Some(owned)), Ok(None)]);
        let lifecycle = Lifecycle::new();
        lifecycle.errors.lock().unwrap().push_back(Some(kind));
        let cfg = config();
        let stop = Notify::new();
        let mut signal = Box::pin(stop.notified());
        let task = worker::run(&repo, &lifecycle, &cfg, &mut signal);
        tokio::pin!(task);
        tokio::select! { _ = &mut task => panic!("stop"), () = repo.admitted.notified() => {} }
        let replace = matches!(
            kind,
            PublishErrorKind::Unavailable | PublishErrorKind::Uncertain
        );
        advance(if replace { cfg.reconnect } else { cfg.failure }).await;
        tokio::select! { _ = &mut task => panic!("stop"), () = repo.admitted.notified() => {} }
        assert_eq!(
            lifecycle.connects.load(Ordering::SeqCst),
            if replace { 2 } else { 1 }
        );
        assert_eq!(
            lifecycle.sends.load(Ordering::SeqCst),
            1,
            "previous claim must not be replayed"
        );
        let tokens = repo.tokens.lock().unwrap().clone();
        assert_eq!(tokens.len(), 2);
        assert_ne!(tokens[0], tokens[1]);
        stop.notify_one();
        task.await;
    }
}
#[tokio::test(start_paused = true)]
async fn preexisting_shutdown_admits_nothing() {
    let repo = Repo::new();
    let lifecycle = Lifecycle::new();
    let cfg = config();
    worker::run(&repo, &lifecycle, &cfg, &mut std::future::ready(())).await;
    assert!(repo.tokens.lock().unwrap().is_empty());
    assert_eq!(lifecycle.connects.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn known_unusable_publisher_is_closed_before_any_acquisition() {
    let repo = Repo::new();
    let mut lifecycle = Lifecycle::new();
    lifecycle.usable = false;
    let cfg = config();
    let stop = Notify::new();
    let mut signal = Box::pin(stop.notified());
    let task = worker::run(&repo, &lifecycle, &cfg, &mut signal);
    tokio::pin!(task);
    tokio::select! { _=&mut task=>panic!("stop"), ()=lifecycle.connected.notified()=>{} }
    assert!(repo.tokens.lock().unwrap().is_empty());
    assert_eq!(lifecycle.closes.load(Ordering::SeqCst), 1);
    stop.notify_one();
    task.await;
    assert_eq!(lifecycle.connects.load(Ordering::SeqCst), 1);
}
