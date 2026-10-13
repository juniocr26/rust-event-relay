use reliable_event_relay::{
    application::{
        delivery::{DeliveryClaim, DeliveryOutcome, FailureHandling},
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
    scans: AtomicUsize,
    tokens: Mutex<Vec<OwnershipToken>>,
    maintenance: Mutex<Vec<(ExhaustedEvent, usize)>>,
    maintenance_error: Option<PersistenceErrorKind>,
    maintenance_block: bool,
    maintenance_entered: Notify,
    responses: Mutex<VecDeque<Result<Option<OwnedOutboxEvent>, PersistenceError>>>,
    admitted: Notify,
    block: AtomicBool,
}
impl Repo {
    fn new() -> Self {
        Self {
            scans: AtomicUsize::new(0),
            tokens: Mutex::new(vec![]),
            maintenance: Mutex::new(vec![]),
            maintenance_error: None,
            maintenance_block: false,
            maintenance_entered: Notify::new(),
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
        retry: reliable_event_relay::domain::retry::RetryState::initial(1).unwrap(),
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
        let handling_results = [
            FailureHandling::LeaseRetained,
            FailureHandling::PolicyFailed(
                reliable_event_relay::application::retry_policy::PolicyError::InvalidSample,
            ),
            FailureHandling::Released(Ok(TransitionOutcome::Applied)),
            FailureHandling::Released(Ok(TransitionOutcome::OwnershipLost)),
            FailureHandling::Released(Err(PersistenceError::new(
                PersistenceErrorKind::CommitUncertain,
            ))),
            FailureHandling::Terminal(Ok(RetryTransition::Applied(PreservedFailure {
                terminal_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                reason:
                    reliable_event_relay::domain::retry::TerminalReason::AcquisitionBudgetExhausted,
                revision: 2,
            }))),
            FailureHandling::Terminal(Ok(RetryTransition::Conflict)),
            FailureHandling::Terminal(Err(PersistenceError::new(
                PersistenceErrorKind::OperationFailed,
            ))),
            FailureHandling::Terminal(Err(PersistenceError::new(
                PersistenceErrorKind::CommitUncertain,
            ))),
            FailureHandling::Scheduled(Ok(RetryTransition::Applied(ScheduledRetry {
                available_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                revision: 1,
            }))),
            FailureHandling::Scheduled(Ok(RetryTransition::Conflict)),
            FailureHandling::Scheduled(Err(PersistenceError::new(
                PersistenceErrorKind::OperationFailed,
            ))),
            FailureHandling::Scheduled(Err(PersistenceError::new(
                PersistenceErrorKind::CommitUncertain,
            ))),
        ];
        for handling in handling_results {
            assert_eq!(
                worker::decision(&DeliveryOutcome::PublicationFailed {
                    claim: claim(),
                    error: PublishError::new(kind),
                    handling
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

impl OutboxRetryScheduler for Repo {
    async fn schedule_retry(
        &self,
        _: ScheduleRetry,
    ) -> Result<RetryTransition<ScheduledRetry>, PersistenceError> {
        Ok(RetryTransition::Applied(ScheduledRetry {
            available_at: chrono::DateTime::from_timestamp(1000, 0).unwrap(),
            revision: 2,
        }))
    }
}
impl OutboxFailurePreserver for Repo {
    async fn preserve_failure(
        &self,
        _: PreserveFailure,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        Ok(RetryTransition::Conflict)
    }
}
impl OutboxExhaustedMaintainer for Repo {
    async fn preserve_exhausted(
        &self,
        candidate: ExhaustedEvent,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        self.maintenance
            .lock()
            .unwrap()
            .push((candidate, self.tokens.lock().unwrap().len()));
        self.maintenance_entered.notify_one();
        if self.maintenance_block {
            pending::<()>().await;
        }
        if let Some(kind) = self.maintenance_error {
            return Err(PersistenceError::new(kind));
        }
        Ok(RetryTransition::Conflict)
    }
}

#[test]
fn maintenance_configuration_is_bounded_and_validated() {
    let a = uuid::Uuid::now_v7();
    for bad in [
        "secret".to_string(),
        format!("{a}:0:0"),
        format!("{a}:1:-1"),
        format!("{a}:1:0,{a}:1:1"),
        format!("{}:1:0", uuid::Uuid::nil()),
        "x".repeat(4097),
    ] {
        assert!(
            WorkerConfig::from_lookup(
                |k| (k == "RELAY_MAINTENANCE_CANDIDATES").then(|| bad.clone())
            )
            .is_err()
        );
    }
    let list = (0..33)
        .map(|_| format!("{}:1:0", uuid::Uuid::now_v7()))
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        WorkerConfig::from_lookup(|k| (k == "RELAY_MAINTENANCE_CANDIDATES").then(|| list.clone()))
            .is_err()
    );
    let list = list.rsplit_once(',').unwrap().0;
    assert_eq!(
        WorkerConfig::from_lookup(|k| (k == "RELAY_MAINTENANCE_CANDIDATES").then(|| list.into()))
            .unwrap()
            .maintenance()
            .len(),
        32
    );
}
#[tokio::test(start_paused = true)]
async fn maintenance_attempts_alternate_with_general_admission_and_never_repeat_stale_candidates() {
    for error in [
        None,
        Some(PersistenceErrorKind::OperationFailed),
        Some(PersistenceErrorKind::CommitUncertain),
    ] {
        let mut repo = Repo::new();
        repo.maintenance_error = error;
        let raw = format!(
            "{}:2:17,{}:3:19",
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7()
        );
        let cfg = WorkerConfig::from_lookup(|k| {
            (k == "RELAY_MAINTENANCE_CANDIDATES").then(|| raw.clone())
        })
        .unwrap();
        let lifecycle = Lifecycle::new();
        let stop = Notify::new();
        let mut signal = Box::pin(stop.notified());
        let mut task = Box::pin(worker::run(&repo, &lifecycle, &cfg, &mut signal));
        for i in 0..3 {
            tokio::select! { _=&mut task=>panic!("stop"), ()=repo.admitted.notified()=>{} }
            if i < 2 {
                advance(cfg.idle).await;
            }
        }
        let attempted = repo.maintenance.lock().unwrap().clone();
        assert_eq!(attempted.len(), 2);
        assert_eq!(attempted[0].0.revision(), 17);
        assert_eq!(attempted[0].1, 0);
        assert_eq!(attempted[1].0.revision(), 19);
        assert_eq!(attempted[1].1, 1);
        stop.notify_one();
        task.await;
    }
}
#[tokio::test(start_paused = true)]
async fn cancelled_maintenance_does_not_admit_delivery_or_followup() {
    let mut repo = Repo::new();
    repo.maintenance_block = true;
    let raw = format!(
        "{}:1:10,{}:1:20",
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7()
    );
    let cfg =
        WorkerConfig::from_lookup(|k| (k == "RELAY_MAINTENANCE_CANDIDATES").then(|| raw.clone()))
            .unwrap();
    let lifecycle = Lifecycle::new();
    let stop = Notify::new();
    let mut signal = Box::pin(stop.notified());
    let mut task = Box::pin(worker::run(&repo, &lifecycle, &cfg, &mut signal));
    tokio::select! {_=&mut task=>panic!("stop"), ()=repo.maintenance_entered.notified()=>{}}
    stop.notify_one();
    task.await;
    assert!(repo.tokens.lock().unwrap().is_empty());
    assert_eq!(repo.maintenance.lock().unwrap().len(), 1);
}

impl OutboxCandidateDiscovery for Repo {
    async fn discover_candidate(
        &self,
        _: DiscoveryCursor,
    ) -> Result<Option<DiscoveryObservation>, PersistenceError> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}
impl OutboxCandidateIsolator for Repo {
    async fn isolate_candidate(
        &self,
        _: MaintenanceCandidate,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        panic!("no candidate")
    }
}

#[tokio::test(start_paused = true)]
async fn discovery_proceeds_during_broker_outage_and_has_no_catchup_burst() {
    let repo = Repo::new();
    let lifecycle = Lifecycle::new();
    lifecycle.fail.store(true, Ordering::SeqCst);
    let stop = Notify::new();
    let mut signal = Box::pin(stop.notified());
    let cfg = config();
    let mut task = Box::pin(worker::run(&repo, &lifecycle, &cfg, &mut signal));
    tokio::select! {_=&mut task=>panic!("stop"),()=lifecycle.connected.notified()=>{}}
    assert_eq!(repo.scans.load(Ordering::SeqCst), 1);
    assert!(repo.tokens.lock().unwrap().is_empty());
    advance(cfg.reconnect * 3).await;
    tokio::select! {_=&mut task=>panic!("stop"),()=lifecycle.connected.notified()=>{}}
    assert_eq!(repo.scans.load(Ordering::SeqCst), 2); // no catchup of missed intervals
    stop.notify_one();
    task.await;
    assert_eq!(repo.scans.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn rapid_delivery_admission_does_not_accelerate_discovery() {
    let repo = Repo::new();
    let lifecycle = Lifecycle::new();
    let stop = Notify::new();
    let cfg =
        WorkerConfig::from_lookup(|key| (key == "RELAY_IDLE_MS").then(|| "20".into())).unwrap();
    let mut signal = Box::pin(stop.notified());
    let mut task = Box::pin(worker::run(&repo, &lifecycle, &cfg, &mut signal));
    for _ in 0..10 {
        tokio::select! {_=&mut task=>panic!("stop"),()=repo.admitted.notified()=>{}}
        assert_eq!(repo.scans.load(Ordering::SeqCst), 1);
        advance(cfg.idle).await;
    }
    assert_eq!(repo.tokens.lock().unwrap().len(), 10);
    stop.notify_one();
    task.await;
}
