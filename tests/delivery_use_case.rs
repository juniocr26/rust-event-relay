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
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Acquire,
    Publish,
    Complete,
    Release,
}
struct Fake {
    event: EventEnvelope,
    calls: Mutex<Vec<Call>>,
    acquired_key: Mutex<Option<OwnedEventKey>>,
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
            calls: Mutex::new(vec![]),
            acquired_key: Mutex::new(None),
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
        let owned =
            OwnedOutboxEvent::new(PendingOutboxEvent::new(self.event.clone(), 7, at), lease)
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
impl OutboxReleaser for Fake {
    async fn release(&self, key: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        assert_eq!(key, self.key());
        self.calls.lock().unwrap().push(Call::Release(key));
        self.boundary(Boundary::Release).await;
        self.transition.map_err(|kind| {
            PersistenceError::with_source(kind, std::io::Error::other("secret-cleanup"))
        })
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
async fn every_publication_failure_preserves_primary_and_cleanup_outcomes() {
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
                release,
            } = outcome
            else {
                panic!("expected publication failure")
            };
            assert_eq!(claim.key.event_id(), fake.event.id());
            assert_eq!(claim.key.token(), token);
            assert_eq!(error.kind(), kind);
            assert!(error.source().unwrap().is::<std::io::Error>());
            if kind == PublishErrorKind::Uncertain {
                assert!(matches!(release, ReleaseOutcome::LeaseRetained));
                assert_eq!(fake.calls(), [Call::Acquire, Call::Publish]);
            } else {
                let ReleaseOutcome::Attempted(result) = release else {
                    panic!("expected release")
                };
                if let Err(error) = &result {
                    assert!(error.source().unwrap().is::<std::io::Error>());
                }
                assert_eq!(result.map_err(|error| error.kind()), transition);
                assert_eq!(
                    fake.calls(),
                    [Call::Acquire, Call::Publish, Call::Release(fake.key())]
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
    ] {
        let mut fake = Fake::new();
        fake.pause = Some(boundary);
        if boundary == Boundary::Release {
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
