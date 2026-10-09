use chrono::{DateTime, Duration, Utc};
use reliable_event_relay::{
    domain::{delivery::*, event::EventEnvelope},
    persistence::*,
};
use uuid::Uuid;
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1000, 0).unwrap()
}
fn duration() -> LeaseDuration {
    LeaseDuration::from_micros(10).unwrap()
}
fn pending(attempts: u32) -> DeliveryState {
    DeliveryState::restore(DeliveryStatus::Pending, attempts, now(), None, None).unwrap()
}
#[test]
fn lease_validation_and_exact_expiry() {
    for value in [i64::MIN, -1, 0] {
        assert_eq!(
            LeaseDuration::from_micros(value),
            Err(DeliveryError::InvalidDuration)
        );
    }
    assert!(LeaseDuration::from_micros(i64::MAX).is_ok());
    assert!(OwnershipToken::restore(Uuid::nil()).is_err());
    let token = OwnershipToken::fresh();
    for end in [
        now(),
        now() - Duration::microseconds(1),
        now() + Duration::nanoseconds(1),
    ] {
        assert!(DeliveryLease::restore(token, now(), end).is_err());
    }
    assert!(DeliveryLease::new(token, DateTime::<Utc>::MAX_UTC, duration()).is_err());
    let lease = DeliveryLease::new(token, now(), duration()).unwrap();
    assert!(!lease.is_active_at(now() - Duration::microseconds(1)));
    assert!(lease.is_active_at(now()));
    assert!(lease.is_active_at(lease.expires_at() - Duration::microseconds(1)));
    assert!(!lease.is_active_at(lease.expires_at()));
}
#[test]
fn recovery_replaces_authority_and_rejects_expired_and_stale_owners() {
    let mut state = pending(0);
    let first = OwnershipToken::fresh();
    let second = OwnershipToken::fresh();
    assert_ne!(first, second);
    let lease = state.acquire(now(), first, duration()).unwrap();
    let before = state.clone();
    assert_eq!(
        state.acquire(now(), second, duration()),
        Err(DeliveryError::NotEligible)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.complete(first, now() - Duration::microseconds(1)),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.complete(first, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.release(first, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.acquire(lease.expires_at(), first, duration()),
        Err(DeliveryError::InvalidToken)
    );
    state
        .acquire(lease.expires_at(), second, duration())
        .unwrap();
    assert_eq!(state.attempt_count(), 2);
    assert_eq!(
        state.release(first, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.complete(first, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    state.complete(second, lease.expires_at()).unwrap();
    assert_eq!(state.status(), DeliveryStatus::Processed);
    assert_eq!(state.processed_at(), Some(lease.expires_at()));
    assert_eq!(state.lease(), None);
    assert_eq!(
        state.complete(second, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.release(second, lease.expires_at()),
        Err(DeliveryError::OwnershipLost)
    );
    assert_eq!(
        state.acquire(lease.expires_at(), OwnershipToken::fresh(), duration()),
        Err(DeliveryError::NotEligible)
    );
}
#[test]
fn release_and_counter_limits_without_partial_mutation() {
    let token = OwnershipToken::fresh();
    let mut state = pending(DeliveryState::MAX_ATTEMPTS - 1);
    state.acquire(now(), token, duration()).unwrap();
    state.release(token, now()).unwrap();
    assert_eq!(state.available_at(), now());
    assert_eq!(state.attempt_count(), DeliveryState::MAX_ATTEMPTS);
    assert_eq!(
        state.release(token, now()),
        Err(DeliveryError::OwnershipLost)
    );
    let before = state.clone();
    assert_eq!(
        state.acquire(now(), OwnershipToken::fresh(), duration()),
        Err(DeliveryError::AttemptLimit)
    );
    assert_eq!(state, before);
    assert!(DeliveryState::restore(DeliveryStatus::Pending, u32::MAX, now(), None, None).is_err());
    let mut future = DeliveryState::restore(
        DeliveryStatus::Pending,
        0,
        now() + Duration::seconds(1),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        future.acquire(now(), token, duration()),
        Err(DeliveryError::NotEligible)
    );
}
#[test]
fn terminal_and_lease_shape_invariants() {
    let lease = DeliveryLease::new(OwnershipToken::fresh(), now(), duration()).unwrap();
    for status in [
        DeliveryStatus::Pending,
        DeliveryStatus::Processed,
        DeliveryStatus::DeadLetter,
    ] {
        for processed in [None, Some(now())] {
            for ownership in [None, Some(lease)] {
                let valid = (status == DeliveryStatus::Processed) == processed.is_some()
                    && (ownership.is_none() || status == DeliveryStatus::Pending);
                assert_eq!(
                    DeliveryState::restore(status, 1, now(), processed, ownership).is_ok(),
                    valid
                );
            }
        }
    }
    assert!(DeliveryState::restore(DeliveryStatus::Pending, 0, now(), None, Some(lease)).is_err());
    let mut dead =
        DeliveryState::restore(DeliveryStatus::DeadLetter, 0, now(), None, None).unwrap();
    assert_eq!(
        dead.acquire(now(), OwnershipToken::fresh(), duration()),
        Err(DeliveryError::NotEligible)
    );
}

// Scripted fake demonstrates generic native Send futures and conflict/error outputs,
// not SQL locking, atomicity or concurrency correctness.
struct Fake;
impl OutboxAcquirer for Fake {
    async fn acquire(
        &self,
        request: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        let event =
            EventEnvelope::new("test", "aggregate", "1", 1, serde_json::Value::Null).unwrap();
        let lease = DeliveryLease::new(request.token(), now(), request.duration()).unwrap();
        Ok(Some(
            OwnedOutboxEvent::new(PendingOutboxEvent::new(event, 1, now()), lease).unwrap(),
        ))
    }
}
impl OutboxCompleter for Fake {
    async fn complete(&self, _: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        Ok(TransitionOutcome::OwnershipLost)
    }
}
impl OutboxReleaser for Fake {
    async fn release(&self, _: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        Err(PersistenceError::new(PersistenceErrorKind::CommitUncertain))
    }
}
fn send<T: Send>(value: T) -> T {
    value
}
async fn use_ports<T: OutboxAcquirer + OutboxCompleter + OutboxReleaser>(ports: &T) {
    let request = AcquireRequest::new(duration());
    let token = request.token();
    let owned = send(ports.acquire(request)).await.unwrap().unwrap();
    assert_eq!(owned.key().event_id(), owned.snapshot().event().id());
    assert_eq!(owned.key().token(), token);
    assert_eq!(
        send(ports.complete(owned.key())).await.unwrap(),
        TransitionOutcome::OwnershipLost
    );
    assert_eq!(
        send(ports.release(owned.key())).await.unwrap_err().kind(),
        PersistenceErrorKind::CommitUncertain
    );
    let envelope = serde_json::to_value(owned.snapshot().event()).unwrap();
    assert_eq!(envelope["id"], owned.key().event_id().to_string());
    assert!(envelope.get("ownership_token").is_none());
    assert!(
        OwnedOutboxEvent::new(
            PendingOutboxEvent::new(owned.snapshot().event().clone(), 0, now()),
            owned.lease()
        )
        .is_err()
    );
    assert!(
        OwnedOutboxEvent::new(
            PendingOutboxEvent::new(
                owned.snapshot().event().clone(),
                1,
                now() + Duration::seconds(1)
            ),
            owned.lease()
        )
        .is_err()
    );
}
#[tokio::test]
async fn contracts_preserve_identity_and_separate_conflicts_from_unknown_commits() {
    use_ports(&Fake).await;
}
