use chrono::{DateTime, Duration, Utc};
use reliable_event_relay::{
    domain::{delivery::OwnershipToken, retry::*},
    persistence::*,
};
use uuid::Uuid;

#[test]
fn history_bounds_consumption_and_revision_never_wrap() {
    for (lifetime, cycle, baseline, max, revision) in [
        (0, 0, 0, 10, 0),
        (0, -1, 0, 10, 0),
        (0, 1, -1, 10, 0),
        (1, 1, 2, 10, 0),
        (0, 1, 0, 0, 0),
        (0, 1, 0, -1, 0),
        (0, 1, 0, 10, -1),
        (u32::MAX, 1, 0, 10, 0),
    ] {
        assert!(RetryState::restore(lifetime, cycle, baseline, max, revision).is_err());
    }
    let state = RetryState::initial(0).unwrap();
    assert_eq!(state.can_acquire(u32::MAX), Err(RetryError::InvalidHistory));
    for (count, allowed) in [(0, true), (9, true), (10, false), (11, false)] {
        assert_eq!(state.consumed(count).unwrap(), count);
        assert_eq!(state.can_acquire(count).unwrap(), allowed);
    }
    let max = RetryState::restore(i32::MAX as u32, i32::MAX, i32::MAX, 10, i64::MAX).unwrap();
    assert!(!max.can_acquire(i32::MAX as u32).unwrap());
    assert_eq!(max.next_revision(), Err(RetryError::RevisionLimit));
    assert!(max.consumed(1).is_err());
}
#[test]
fn delays_reject_precision_range_and_checked_timestamp_overflow() {
    for delay in [-1, 0, 86_400_000_001, i64::MAX] {
        assert!(RetryDelay::from_micros(delay).is_err());
    }
    assert!(RetryDelay::from_duration(Duration::nanoseconds(1001)).is_err());
    let delay = RetryDelay::from_micros(1).unwrap();
    assert!(
        delay
            .available_at(DateTime::<Utc>::from_timestamp(1, 1).unwrap())
            .is_err()
    );
    let max =
        DateTime::<Utc>::from_timestamp(DateTime::<Utc>::MAX_UTC.timestamp(), 999_999_000).unwrap();
    assert!(delay.available_at(max).is_err());
    assert!(delay.available_at(DateTime::<Utc>::MIN_UTC).is_err());
    let now = DateTime::<Utc>::from_timestamp(1, 0).unwrap();
    assert_eq!(
        delay.available_at(now).unwrap() - now,
        Duration::microseconds(1)
    );
    assert_eq!(
        RetryDelay::from_duration(Duration::days(1))
            .unwrap()
            .as_micros(),
        86_400_000_000
    );
}
#[test]
fn evidence_has_no_free_text_and_requests_validate_identity() {
    assert!(FailureEvidence::new(FailurePhase::BeforeSend, true, Some(Uuid::nil())).is_err());
    let evidence =
        FailureEvidence::new(FailurePhase::BrokerResponse, true, Some(Uuid::from_u128(1)))
            .unwrap()
            .to_json();
    assert!(serde_json::to_vec(&evidence).unwrap().len() < 2048);
    assert_eq!(evidence.as_object().unwrap().len(), 4);
    assert!(ExhaustedEvent::new(Uuid::nil(), 1, 0).is_err());
    assert!(ExhaustedEvent::new(Uuid::from_u128(1), 0, 0).is_err());
    assert!(ExhaustedEvent::new(Uuid::from_u128(1), 1, -1).is_err());
    assert!(
        RetryOwner::new(
            OwnedEventKey::new(Uuid::nil(), OwnershipToken::fresh()),
            RetryState::initial(0).unwrap()
        )
        .is_err()
    );
}
