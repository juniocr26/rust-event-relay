use chrono::{DateTime, Utc};
use reliable_event_relay::{
    application::{delivery::*, publisher::*},
    domain::{delivery::*, event::EventEnvelope},
    persistence::*,
};
use std::{error::Error, sync::Mutex};
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Acquire,
    Publish,
    Complete(OwnedEventKey),
    Release(OwnedEventKey),
    Schedule(OwnedEventKey),
    Terminal(OwnedEventKey),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Acquire,
    Publish,
    Complete,
    Release,
    Schedule,
    Terminal,
}
struct Fake {
    event: EventEnvelope,
    count: u32,
    state: reliable_event_relay::domain::retry::RetryState,
    permanent: bool,
    calls: Mutex<Vec<Call>>,
    acquired_key: Mutex<Option<OwnedEventKey>>,
    scheduled: Mutex<Option<ScheduleRetry>>,
    acquisition: Result<bool, PersistenceErrorKind>,
    publication: Result<(), PublishErrorKind>,
    transition: Result<TransitionOutcome, PersistenceErrorKind>,
    pause: Option<Boundary>,
    entered: Notify,
    resume: Notify,
}
impl Fake {
    fn new() -> Self {
        Self {
            event: EventEnvelope::new(
                "created",
                "account",
                "original",
                2,
                serde_json::json!({"secret-payload": [1, true, "value"]}),
            )
            .unwrap()
            .with_correlation_id(uuid::Uuid::now_v7())
            .with_causation_id(uuid::Uuid::now_v7()),
            count: 7,
            state: reliable_event_relay::domain::retry::RetryState::restore(7, 2, 3, 10, 17)
                .unwrap(),
            permanent: false,
            calls: Mutex::new(vec![]),
            acquired_key: Mutex::new(None),
            scheduled: Mutex::new(None),
            acquisition: Ok(true),
            publication: Ok(()),
            transition: Ok(TransitionOutcome::Applied),
            pause: None,
            entered: Notify::new(),
            resume: Notify::new(),
        }
    }
    async fn boundary(&self, at: Boundary) {
        if self.pause == Some(at) {
            self.entered.notify_one();
            self.resume.notified().await;
        }
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn key(&self) -> OwnedEventKey {
        self.acquired_key.lock().unwrap().unwrap()
    }
}
impl OutboxAcquirer for Fake {
    async fn acquire(
        &self,
        request: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        self.calls.lock().unwrap().push(Call::Acquire);
        self.boundary(Boundary::Acquire).await;
        if !self.acquisition.map_err(PersistenceError::new)? {
            return Ok(None);
        }
        // Deliberately expired relative to wall time: orchestration does not authorize
        // transitions using its own clock or silently skip this acknowledged claim.
        let at = DateTime::<Utc>::from_timestamp(1000, 0).unwrap();
        let lease = DeliveryLease::new(request.token(), at, request.duration()).unwrap();
        let owned = OwnedOutboxEvent::new(
            PendingOutboxEvent::new(self.event.clone(), self.count, at),
            lease,
        )
        .unwrap()
        .with_retry_state(self.state)
        .unwrap();
        *self.acquired_key.lock().unwrap() = Some(owned.key());
        Ok(Some(owned))
    }
}
impl EventPublisher for Fake {
    async fn publish(&self, event: &EventEnvelope) -> Result<(), PublishError> {
        assert_eq!(event, &self.event); // all identity, time, payload and metadata
        self.calls.lock().unwrap().push(Call::Publish);
        self.boundary(Boundary::Publish).await;
        if self.permanent {
            return Err(PublishError::permanent_serialization(
                reliable_event_relay::domain::retry::FailureEvidence::new(
                    reliable_event_relay::domain::retry::FailurePhase::BeforeSend,
                    true,
                    Some(uuid::Uuid::now_v7()),
                )
                .unwrap(),
            ));
        }
        self.publication
            .map_err(|kind| PublishError::with_source(kind, std::io::Error::other("secret-source")))
    }
}
impl OutboxCompleter for Fake {
    async fn complete(&self, key: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        assert_eq!(key, self.key());
        self.calls.lock().unwrap().push(Call::Complete(key));
        self.boundary(Boundary::Complete).await;
        self.transition.map_err(PersistenceError::new)
    }
}
impl OutboxRetryScheduler for Fake {
    async fn schedule_retry(
        &self,
        request: ScheduleRetry,
    ) -> Result<RetryTransition<ScheduledRetry>, PersistenceError> {
        assert_eq!(request.owner.key(), self.key());
        *self.scheduled.lock().unwrap() = Some(request);
        assert_eq!(request.owner.cycle(), self.state.cycle());
        assert_eq!(request.owner.revision(), self.state.revision());
        let ceiling = reliable_event_relay::application::retry_policy::RetryBackoff::new(
            1_000_000,
            300_000_000,
        )
        .unwrap()
        .ceiling_micros(self.count - self.state.baseline() as u32)
        .unwrap();
        assert!((ceiling / 2..=ceiling).contains(&request.delay.as_micros()));
        self.calls.lock().unwrap().push(Call::Schedule(self.key()));
        self.boundary(Boundary::Schedule).await;
        match self.transition.map_err(|kind| {
            PersistenceError::with_source(kind, std::io::Error::other("secret-cleanup"))
        })? {
            TransitionOutcome::Applied => Ok(RetryTransition::Applied(ScheduledRetry {
                available_at: DateTime::from_timestamp(2000, 0).unwrap(),
                revision: self.state.revision() + 1,
            })),
            TransitionOutcome::OwnershipLost => Ok(RetryTransition::Conflict),
        }
    }
}
impl OutboxFailurePreserver for Fake {
    async fn preserve_failure(
        &self,
        request: PreserveFailure,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        assert_eq!(request.owner.key(), self.key());
        assert_eq!(request.owner.cycle(), self.state.cycle());
        assert_eq!(request.owner.revision(), self.state.revision());
        self.calls.lock().unwrap().push(Call::Terminal(self.key()));
        self.boundary(Boundary::Terminal).await;
        match self.transition.map_err(PersistenceError::new)? {
            TransitionOutcome::Applied => Ok(RetryTransition::Applied(PreservedFailure {
                terminal_at: DateTime::from_timestamp(2000, 0).unwrap(),
                reason: request.reason,
                revision: self.state.revision() + 1,
            })),
            TransitionOutcome::OwnershipLost => Ok(RetryTransition::Conflict),
        }
    }
}
fn request() -> AcquireRequest {
    AcquireRequest::new(LeaseDuration::from_micros(100).unwrap())
}
fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn empty_or_failed_acquisition_has_no_subsequent_effect() {
    let mut fake = Fake::new();
    fake.acquisition = Ok(false);
    assert!(matches!(
        deliver_once(&fake, &fake, request()).await,
        DeliveryOutcome::NoEvent
    ));
    assert_eq!(fake.calls(), [Call::Acquire]);
    for kind in [
        PersistenceErrorKind::Unavailable,
        PersistenceErrorKind::InvalidStoredData,
        PersistenceErrorKind::OperationFailed,
        PersistenceErrorKind::CommitUncertain,
    ] {
        let mut fake = Fake::new();
        fake.acquisition = Err(kind);
        let req = request();
        let token = req.token();
        let duration = req.duration();
        let DeliveryOutcome::AcquisitionFailed {
            token: actual,
            duration: actual_duration,
            error,
        } = deliver_once(&fake, &fake, req).await
        else {
            panic!("expected acquisition failure")
        };
        assert_eq!(actual, token);
        assert_eq!(actual_duration, duration);
        assert_eq!(error.kind(), kind);
        assert_eq!(fake.calls(), [Call::Acquire]);
        assert!(fake.acquired_key.lock().unwrap().is_none());
    }
}

#[tokio::test]
async fn confirmed_publication_then_every_completion_result_without_cleanup_or_retry() {
    for transition in [
        Ok(TransitionOutcome::Applied),
        Ok(TransitionOutcome::OwnershipLost),
        Err(PersistenceErrorKind::Unavailable),
        Err(PersistenceErrorKind::InvalidStoredData),
        Err(PersistenceErrorKind::OperationFailed),
        Err(PersistenceErrorKind::CommitUncertain),
    ] {
        let mut fake = Fake::new();
        fake.transition = transition;
        let req = request();
        let token = req.token();
        let future = deliver_once(&fake, &fake, req);
        assert_send(&future);
        let outcome = future.await;
        assert!(!format!("{outcome:?}").contains("secret-payload"));
        let DeliveryOutcome::Published { claim, completion } = outcome else {
            panic!("expected confirmation")
        };
        assert_eq!(claim.key.event_id(), fake.event.id());
        assert_eq!(claim.key.token(), token);
        assert_eq!(claim.lease.token(), token);
        assert_eq!(claim.attempt_count, 7);
        assert_eq!(completion.map_err(|error| error.kind()), transition);
        assert_eq!(
            fake.calls(),
            [Call::Acquire, Call::Publish, Call::Complete(fake.key())]
        );
    }
}

#[tokio::test]
async fn every_publication_failure_preserves_primary_and_handling_outcomes() {
    for kind in [
        PublishErrorKind::Unavailable,
        PublishErrorKind::Serialization,
        PublishErrorKind::Unroutable,
        PublishErrorKind::Rejected,
        PublishErrorKind::Uncertain,
    ] {
        for transition in [
            Ok(TransitionOutcome::Applied),
            Ok(TransitionOutcome::OwnershipLost),
            Err(PersistenceErrorKind::Unavailable),
            Err(PersistenceErrorKind::InvalidStoredData),
            Err(PersistenceErrorKind::Unavailable),
            Err(PersistenceErrorKind::InvalidStoredData),
            Err(PersistenceErrorKind::OperationFailed),
            Err(PersistenceErrorKind::CommitUncertain),
        ] {
            let mut fake = Fake::new();
            fake.publication = Err(kind);
            fake.transition = transition;
            let req = request();
            let token = req.token();
            let outcome = deliver_once(&fake, &fake, req).await;
            let printed = format!("{outcome:?}");
            assert!(!printed.contains("secret"));
            let DeliveryOutcome::PublicationFailed {
                claim,
                error,
                handling,
            } = outcome
            else {
                panic!("expected publication failure")
            };
            assert_eq!(claim.key.event_id(), fake.event.id());
            assert_eq!(claim.key.token(), token);
            assert_eq!(error.kind(), kind);
            assert!(error.source().unwrap().is::<std::io::Error>());
            if matches!(
                kind,
                PublishErrorKind::Uncertain | PublishErrorKind::Serialization
            ) {
                assert!(matches!(handling, FailureHandling::LeaseRetained));
                assert_eq!(fake.calls(), [Call::Acquire, Call::Publish]);
            } else if kind == PublishErrorKind::Unavailable {
                let FailureHandling::Released(result) = handling else {
                    panic!("infrastructure release")
                };
                assert_eq!(result.map_err(|e| e.kind()), transition);
                assert_eq!(
                    fake.calls(),
                    [Call::Acquire, Call::Publish, Call::Release(fake.key())]
                );
            } else {
                let FailureHandling::Scheduled(result) = handling else {
                    panic!("expected release")
                };
                if let Err(error) = &result {
                    assert!(error.source().unwrap().is::<std::io::Error>());
                }
                assert_eq!(
                    result
                        .map(|r| match r {
                            RetryTransition::Applied(_) => TransitionOutcome::Applied,
                            RetryTransition::Conflict => TransitionOutcome::OwnershipLost,
                        })
                        .map_err(|error| error.kind()),
                    transition
                );
                assert_eq!(
                    fake.calls(),
                    [Call::Acquire, Call::Publish, Call::Schedule(fake.key())]
                );
            }
        }
    }
}

#[tokio::test]
async fn confirmation_is_required_and_cancellation_stops_subsequent_calls() {
    for boundary in [
        Boundary::Acquire,
        Boundary::Publish,
        Boundary::Complete,
        Boundary::Release,
        Boundary::Schedule,
        Boundary::Terminal,
    ] {
        let mut fake = Fake::new();
        fake.pause = Some(boundary);
        if boundary == Boundary::Release {
            fake.publication = Err(PublishErrorKind::Unavailable);
        }
        if boundary == Boundary::Terminal {
            fake.permanent = true;
        }
        if boundary == Boundary::Schedule {
            fake.publication = Err(PublishErrorKind::Rejected);
        }
        // Poll the owned boxed future until the fake explicitly signals its boundary.
        // Dropping actually cancels it; dropping a Pin<&mut Future> alone would not.
        let mut future = Box::pin(deliver_once(&fake, &fake, request()));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::select! {
                _ = fake.entered.notified() => {},
                _ = &mut future => panic!("boundary should be blocked"),
            }
        })
        .await
        .unwrap();
        let expected = match boundary {
            Boundary::Acquire => vec![Call::Acquire],
            Boundary::Publish => vec![Call::Acquire, Call::Publish],
            Boundary::Complete => vec![Call::Acquire, Call::Publish, Call::Complete(fake.key())],
            Boundary::Release => vec![Call::Acquire, Call::Publish, Call::Release(fake.key())],
            Boundary::Schedule => vec![Call::Acquire, Call::Publish, Call::Schedule(fake.key())],
            Boundary::Terminal => vec![Call::Acquire, Call::Publish, Call::Terminal(fake.key())],
        };
        assert_eq!(fake.calls(), expected);
        drop(future);
        fake.resume.notify_one();
        tokio::task::yield_now().await;
        assert_eq!(fake.calls(), expected);
        // These fakes cannot establish real rollback or broker nonacceptance.
    }
}

#[tokio::test]
async fn completion_waits_for_explicit_publisher_confirmation() {
    let mut fake = Fake::new();
    fake.pause = Some(Boundary::Publish);
    let mut future = Box::pin(deliver_once(&fake, &fake, request()));
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::select! {
            _ = fake.entered.notified() => {},
            _ = &mut future => panic!("publication must wait"),
        }
    })
    .await
    .unwrap();
    assert_eq!(fake.calls(), [Call::Acquire, Call::Publish]);
    fake.resume.notify_one();
    assert!(matches!(
        future.await,
        DeliveryOutcome::Published {
            completion: Ok(TransitionOutcome::Applied),
            ..
        }
    ));
    assert_eq!(
        fake.calls(),
        [Call::Acquire, Call::Publish, Call::Complete(fake.key())]
    );
}

#[tokio::test]
async fn final_and_permanent_failures_terminalize_with_independent_results() {
    use reliable_event_relay::domain::retry::*;
    for permanent in [false, true] {
        for transition in [
            Ok(TransitionOutcome::Applied),
            Ok(TransitionOutcome::OwnershipLost),
            Err(PersistenceErrorKind::Unavailable),
            Err(PersistenceErrorKind::InvalidStoredData),
            Err(PersistenceErrorKind::OperationFailed),
            Err(PersistenceErrorKind::CommitUncertain),
        ] {
            let mut fake = Fake::new();
            fake.permanent = permanent;
            fake.publication = Err(PublishErrorKind::Unroutable);
            fake.count = 13;
            fake.state = RetryState::restore(13, 2, 3, 10, 42).unwrap();
            fake.transition = transition;
            let outcome = deliver_once_with_delay(&fake, &fake, request(), |_| {
                panic!("terminal decisions do not sample")
            })
            .await;
            let DeliveryOutcome::PublicationFailed {
                error,
                handling: FailureHandling::Terminal(result),
                claim,
            } = outcome
            else {
                panic!("terminal outcome")
            };
            assert_eq!(claim.retry, fake.state);
            assert_eq!(
                error.kind(),
                if permanent {
                    PublishErrorKind::Serialization
                } else {
                    PublishErrorKind::Unroutable
                }
            );
            assert_eq!(
                result
                    .as_ref()
                    .map(|r| match r {
                        RetryTransition::Applied(_) => TransitionOutcome::Applied,
                        RetryTransition::Conflict => TransitionOutcome::OwnershipLost,
                    })
                    .map_err(|e| e.kind()),
                transition
            );
            if let Ok(RetryTransition::Applied(value)) = result {
                assert_eq!(
                    value.reason,
                    if permanent {
                        TerminalReason::SerializationPermanent
                    } else {
                        TerminalReason::AcquisitionBudgetExhausted
                    }
                );
            }
            assert_eq!(
                fake.calls(),
                [Call::Acquire, Call::Publish, Call::Terminal(fake.key())]
            );
        }
    }
}
#[tokio::test]
async fn final_uncertainty_retains_and_scheduling_samples_once_or_reports_policy_failure() {
    let mut fake = Fake::new();
    fake.count = 13;
    fake.state =
        reliable_event_relay::domain::retry::RetryState::restore(13, 2, 3, 10, 42).unwrap();
    fake.publication = Err(PublishErrorKind::Uncertain);
    assert!(matches!(
        deliver_once_with_delay(&fake, &fake, request(), |_| panic!("no sample")).await,
        DeliveryOutcome::PublicationFailed {
            handling: FailureHandling::LeaseRetained,
            ..
        }
    ));
    assert_eq!(fake.calls(), [Call::Acquire, Call::Publish]);
    let mut fake = Fake::new();
    fake.publication = Err(PublishErrorKind::Rejected);
    let mut samples = 0;
    let outcome = deliver_once_with_delay(&fake, &fake, request(), |n| {
        samples += 1;
        assert_eq!(n, 4);
        reliable_event_relay::application::retry_policy::selected_delay(n, 0)
    })
    .await;
    assert_eq!(samples, 1);
    let selected = fake.scheduled.lock().unwrap().unwrap();
    assert_eq!(selected.delay.as_micros(), 4_000_000);
    assert_eq!(
        selected.diagnostic,
        reliable_event_relay::domain::retry::RetryDiagnostic::Rejected
    );
    assert_eq!(selected.evidence.to_json()["phase"], "broker_response");
    assert!(matches!(
        outcome,
        DeliveryOutcome::PublicationFailed {
            handling: FailureHandling::Scheduled(Ok(RetryTransition::Applied(_))),
            ..
        }
    ));
    let mut fake = Fake::new();
    fake.publication = Err(PublishErrorKind::Rejected);
    assert!(matches!(
        deliver_once_with_delay(&fake, &fake, request(), |_| Err(
            reliable_event_relay::application::retry_policy::PolicyError::InvalidSample
        ))
        .await,
        DeliveryOutcome::PublicationFailed {
            handling: FailureHandling::PolicyFailed(_),
            ..
        }
    ));
    assert_eq!(fake.calls(), [Call::Acquire, Call::Publish]);
}

impl OutboxReleaser for Fake {
    async fn release(&self, key: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        assert_eq!(key, self.key());
        self.calls.lock().unwrap().push(Call::Release(key));
        self.boundary(Boundary::Release).await;
        self.transition.map_err(PersistenceError::new)
    }
}

#[tokio::test]
async fn every_definitive_final_failure_stops_while_final_acceptance_can_complete() {
    use reliable_event_relay::domain::retry::*;
    for kind in [
        PublishErrorKind::Unavailable,
        PublishErrorKind::Serialization,
        PublishErrorKind::Rejected,
        PublishErrorKind::Unroutable,
    ] {
        let mut fake = Fake::new();
        fake.count = 13;
        fake.state = RetryState::restore(13, 2, 3, 10, 42).unwrap();
        fake.publication = Err(kind);
        let outcome = deliver_once_with_delay(&fake, &fake, request(), |_| {
            panic!("no schedule on final slot")
        })
        .await;
        assert!(
            matches!(outcome,DeliveryOutcome::PublicationFailed {error,handling:FailureHandling::Terminal(Ok(RetryTransition::Applied(value))),..} if error.kind()==kind && value.reason==TerminalReason::AcquisitionBudgetExhausted)
        );
        assert_eq!(
            fake.calls(),
            [Call::Acquire, Call::Publish, Call::Terminal(fake.key())]
        );
    }
    let mut fake = Fake::new();
    fake.count = 13;
    fake.state = RetryState::restore(13, 2, 3, 10, 42).unwrap();
    assert!(matches!(
        deliver_once_with_delay(&fake, &fake, request(), |_| panic!(
            "confirmed final completes"
        ))
        .await,
        DeliveryOutcome::Published {
            completion: Ok(TransitionOutcome::Applied),
            ..
        }
    ));
    assert_eq!(
        fake.calls(),
        [Call::Acquire, Call::Publish, Call::Complete(fake.key())]
    );
    let mut fake = Fake::new();
    fake.count = i32::MAX as u32;
    fake.state = RetryState::restore(fake.count, 2, i32::MAX - 1, 10, 42).unwrap();
    fake.publication = Err(PublishErrorKind::Rejected);
    assert!(
        matches!(deliver_once(&fake,&fake,request()).await,DeliveryOutcome::PublicationFailed {handling:FailureHandling::Terminal(Ok(RetryTransition::Applied(value))),..} if value.reason==TerminalReason::StorageCounterExhausted)
    );
}

#[tokio::test]
async fn observations_keep_confirm_even_when_completion_is_uncertain_or_cancelled() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        telemetry::metrics::{Metrics, Process},
    };
    let metrics = Metrics::new(Process::Relay);
    let mut fake = Fake::new();
    fake.transition = Err(PersistenceErrorKind::CommitUncertain);
    let request = || AcquireRequest::new(LeaseDuration::from_micros(1_000_000).unwrap());
    let result = deliver_once_observed(
        &fake,
        &fake,
        request(),
        reliable_event_relay::application::retry_policy::sample_delay,
        metrics.as_ref(),
    )
    .await;
    assert!(matches!(
        result,
        DeliveryOutcome::Published {
            completion: Err(_),
            ..
        }
    ));
    assert_eq!(metrics.count(Op::Acquisition, Obs::Acquired), 1);
    assert_eq!(metrics.count(Op::Publication, Obs::Confirmed), 1);
    assert_eq!(metrics.count(Op::Completion, Obs::CommitUncertain), 1);
    assert_eq!(metrics.count(Op::Completion, Obs::Applied), 0);
    fake.pause = Some(Boundary::Complete);
    let mut operation = Box::pin(deliver_once_observed(
        &fake,
        &fake,
        request(),
        reliable_event_relay::application::retry_policy::sample_delay,
        metrics.as_ref(),
    ));
    assert!(futures_util::poll!(&mut operation).is_pending());
    assert_eq!(metrics.count(Op::Publication, Obs::Confirmed), 2);
    assert_eq!(metrics.in_flight(Op::Completion), 1);
    drop(operation);
    assert_eq!(metrics.in_flight(Op::Completion), 0);
    assert_eq!(metrics.count(Op::Completion, Obs::Cancelled), 1);
    assert_eq!(metrics.count(Op::Completion, Obs::Applied), 0);
}

#[tokio::test]
async fn durable_schedule_and_terminal_metrics_require_acknowledged_applied_transition() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        telemetry::metrics::{Metrics, Process},
    };
    for terminal in [false, true] {
        for (transition, label) in [
            (Ok(TransitionOutcome::Applied), Obs::Applied),
            (Ok(TransitionOutcome::OwnershipLost), Obs::Conflict),
            (
                Err(PersistenceErrorKind::CommitUncertain),
                Obs::CommitUncertain,
            ),
        ] {
            let metrics = Metrics::new(Process::Relay);
            let mut fake = Fake::new();
            fake.publication = Err(PublishErrorKind::Rejected);
            fake.transition = transition;
            if terminal {
                fake.count = u32::try_from(fake.state.baseline() + fake.state.maximum()).unwrap();
            }
            deliver_once_observed(
                &fake,
                &fake,
                request(),
                reliable_event_relay::application::retry_policy::sample_delay,
                metrics.as_ref(),
            )
            .await;
            let op = if terminal { Op::Terminal } else { Op::Schedule };
            assert_eq!(metrics.count(op, label), 1);
            assert_eq!(metrics.count(Op::Publication, Obs::Confirmed), 0);
            if !matches!(transition, Ok(TransitionOutcome::Applied)) {
                assert_eq!(metrics.count(op, Obs::Applied), 0);
            }
        }
    }
}

#[tokio::test]
async fn acquisition_observations_do_not_infer_claims_or_publications_on_failure() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        telemetry::metrics::{Metrics, Process},
    };
    for (acquisition, label) in [
        (Ok(false), Obs::Empty),
        (
            Err(PersistenceErrorKind::CommitUncertain),
            Obs::CommitUncertain,
        ),
        (Err(PersistenceErrorKind::Unavailable), Obs::Unavailable),
    ] {
        let metrics = Metrics::new(Process::Relay);
        let mut fake = Fake::new();
        fake.acquisition = acquisition;
        deliver_once_observed(
            &fake,
            &fake,
            request(),
            reliable_event_relay::application::retry_policy::sample_delay,
            metrics.as_ref(),
        )
        .await;
        assert_eq!(metrics.count(Op::Acquisition, label), 1);
        assert_eq!(metrics.count(Op::Acquisition, Obs::Acquired), 0);
        assert_eq!(metrics.count(Op::Publication, Obs::Confirmed), 0);
        assert_eq!(fake.calls(), vec![Call::Acquire]);
    }
}
