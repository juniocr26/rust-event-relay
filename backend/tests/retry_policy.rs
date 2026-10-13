use chrono::{DateTime, Utc};
use reliable_event_relay::application::retry_policy::*;

#[test]
fn budget_counts_acquisitions_preserves_history_and_distinguishes_storage_limit() {
    let budget = AcquisitionBudget::new(10).unwrap();
    assert_eq!(budget.remaining(0, 0), Ok(10));
    assert_eq!(budget.remaining(9, 0), Ok(1));
    assert_eq!(budget.remaining(10, 0), Ok(0));
    assert_eq!(budget.remaining(11, 0), Ok(0));
    assert_eq!(budget.remaining(110, 100), Ok(0));
    assert_eq!(budget.remaining(110, 110), Ok(10));
    assert_eq!(budget.remaining(9, 10), Err(PolicyError::InvalidHistory));
    assert_eq!(
        budget.remaining(u32::MAX, 0),
        Err(PolicyError::InvalidHistory)
    );
    assert!(
        !budget
            .can_acquire(i32::MAX as u32, i32::MAX as u32)
            .unwrap()
    );
    assert!(AcquisitionBudget::new(0).is_err());
    assert!(AcquisitionBudget::new(u32::MAX).is_err());
}

#[test]
fn exponential_ceiling_and_equal_jitter_have_exact_bounded_endpoints() {
    let policy = RetryBackoff::new(3, 20).unwrap();
    let now = DateTime::<Utc>::from_timestamp(1000, 0).unwrap();
    for (attempt, ceiling) in [(1, 3), (2, 6), (3, 12), (4, 20), (i32::MAX as u32, 20)] {
        assert_eq!(policy.ceiling_micros(attempt), Ok(ceiling));
        let floor = ceiling / 2 + ceiling % 2;
        for offset in 0..=ceiling - floor {
            let at = policy.available_at(now, attempt, offset).unwrap();
            assert_eq!((at - now).num_microseconds(), Some(floor + offset));
        }
        assert_eq!(
            policy.available_at(now, attempt, -1),
            Err(PolicyError::InvalidSample)
        );
        assert_eq!(
            policy.available_at(now, attempt, ceiling - floor + 1),
            Err(PolicyError::InvalidSample)
        );
    }
}

#[test]
fn invalid_bounds_precision_and_timestamp_overflow_are_rejected() {
    for (base, cap) in [
        (0, 1),
        (-1, 2),
        (3, 2),
        (1, 86_400_000_001),
        (i64::MAX, i64::MAX),
    ] {
        assert!(RetryBackoff::new(base, cap).is_err());
    }
    let policy = RetryBackoff::new(1, 1).unwrap();
    assert_eq!(policy.ceiling_micros(0), Err(PolicyError::InvalidHistory));
    assert_eq!(
        policy.ceiling_micros(u32::MAX),
        Err(PolicyError::InvalidHistory)
    );
    assert_eq!(
        policy.available_at(DateTime::<Utc>::from_timestamp(1, 1).unwrap(), 1, 0),
        Err(PolicyError::InvalidTimestamp)
    );
    let max = DateTime::<Utc>::MAX_UTC;
    let max = DateTime::<Utc>::from_timestamp(max.timestamp(), 999_999_000).unwrap();
    assert_eq!(
        policy.available_at(max, 1, 0),
        Err(PolicyError::InvalidTimestamp)
    );
}

#[test]
fn integrated_equal_jitter_has_inclusive_microsecond_endpoints() {
    use reliable_event_relay::application::retry_policy::{
        PolicyError, sample_delay, selected_delay,
    };
    for (n, floor, ceiling) in [
        (1, 500_000, 1_000_000),
        (2, 1_000_000, 2_000_000),
        (10, 150_000_000, 300_000_000),
        (i32::MAX as u32, 150_000_000, 300_000_000),
    ] {
        assert_eq!(selected_delay(n, 0).unwrap().as_micros(), floor);
        assert_eq!(
            selected_delay(n, ceiling - floor).unwrap().as_micros(),
            ceiling
        );
        assert_eq!(selected_delay(n, -1), Err(PolicyError::InvalidSample));
        assert_eq!(
            selected_delay(n, ceiling - floor + 1),
            Err(PolicyError::InvalidSample)
        );
        assert!((floor..=ceiling).contains(&sample_delay(n).unwrap().as_micros()));
    }
    assert!(selected_delay(0, 0).is_err());
    assert!(selected_delay(u32::MAX, 0).is_err());
}
