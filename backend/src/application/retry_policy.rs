//! Pure backoff calculations with a separate production equal-jitter sampler.
use crate::domain::delivery::DeliveryState;
use chrono::{DateTime, Duration, Utc};

/// Invalid configuration/history, randomness, precision, or checked time arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    InvalidBounds,
    InvalidHistory,
    InvalidSample,
    InvalidTimestamp,
}

/// A cycle consumes committed acquisitions, including crashes before publication.
#[derive(Debug, Clone, Copy)]
pub struct AcquisitionBudget {
    maximum: u32,
}
impl AcquisitionBudget {
    pub fn new(maximum: u32) -> Result<Self, PolicyError> {
        if maximum == 0 || maximum > DeliveryState::MAX_ATTEMPTS {
            return Err(PolicyError::InvalidBounds);
        }
        Ok(Self { maximum })
    }
    /// Baseline is the lifetime count recorded at cycle start, never a reset count.
    pub fn remaining(self, lifetime: u32, baseline: u32) -> Result<u32, PolicyError> {
        if lifetime > DeliveryState::MAX_ATTEMPTS || baseline > lifetime {
            return Err(PolicyError::InvalidHistory);
        }
        let used = lifetime - baseline;
        // Clamp only the derived allowance to zero; lifetime history is unchanged.
        Ok(self.maximum.saturating_sub(used))
    }
    /// Storage exhaustion is independent of the configured cycle allowance.
    pub fn can_acquire(self, lifetime: u32, baseline: u32) -> Result<bool, PolicyError> {
        Ok(self.remaining(lifetime, baseline)? > 0 && lifetime < DeliveryState::MAX_ATTEMPTS)
    }
}

/// Positive whole-microsecond delays, capped at one day; equal jitter.
#[derive(Debug, Clone, Copy)]
pub struct RetryBackoff {
    base_micros: i64,
    cap_micros: i64,
}
impl RetryBackoff {
    pub fn new(base_micros: i64, cap_micros: i64) -> Result<Self, PolicyError> {
        if base_micros <= 0 || cap_micros < base_micros || cap_micros > 86_400_000_000 {
            return Err(PolicyError::InvalidBounds);
        }
        Ok(Self {
            base_micros,
            cap_micros,
        })
    }
    /// Ceiling for acquisition number within a cycle (one-based).
    pub fn ceiling_micros(self, acquisition: u32) -> Result<i64, PolicyError> {
        if acquisition == 0 || acquisition > DeliveryState::MAX_ATTEMPTS {
            return Err(PolicyError::InvalidHistory);
        }
        let mut ceiling = self.base_micros;
        for _ in 1..acquisition {
            if ceiling == self.cap_micros {
                break;
            }
            // Validated cap makes doubling representable; cap is intentional policy.
            ceiling = ceiling
                .checked_mul(2)
                .ok_or(PolicyError::InvalidBounds)?
                .min(self.cap_micros);
        }
        Ok(ceiling)
    }
    /// Caller supplies a uniform integer offset in 0..=ceiling-floor.
    /// Production randomness must use unbiased bounded sampling, not modulo reduction.
    pub fn available_at(
        self,
        now: DateTime<Utc>,
        acquisition: u32,
        offset: i64,
    ) -> Result<DateTime<Utc>, PolicyError> {
        if !now.timestamp_subsec_nanos().is_multiple_of(1000) {
            return Err(PolicyError::InvalidTimestamp);
        }
        let ceiling = self.ceiling_micros(acquisition)?;
        let floor = ceiling / 2 + ceiling % 2;
        if offset < 0 || offset > ceiling - floor {
            return Err(PolicyError::InvalidSample);
        }
        let delay = floor
            .checked_add(offset)
            .ok_or(PolicyError::InvalidBounds)?;
        now.checked_add_signed(Duration::microseconds(delay))
            .ok_or(PolicyError::InvalidTimestamp)
    }
}

/// ADR 008: equal jitter on the inclusive whole-microsecond interval.
/// Sampling is separate so callers can inject exact boundary offsets in tests.
pub fn selected_delay(
    acquisition: u32,
    offset: i64,
) -> Result<crate::domain::retry::RetryDelay, PolicyError> {
    let ceiling = RetryBackoff::new(1_000_000, 300_000_000)?.ceiling_micros(acquisition)?;
    let floor = ceiling / 2 + ceiling % 2;
    if !(0..=ceiling - floor).contains(&offset) {
        return Err(PolicyError::InvalidSample);
    }
    let micros = floor
        .checked_add(offset)
        .ok_or(PolicyError::InvalidBounds)?;
    crate::domain::retry::RetryDelay::from_micros(micros).map_err(|_| PolicyError::InvalidBounds)
}
/// Thread-local OS-seeded rand 0.8 generator; gen_range uses unbiased bounded
/// sampling, never naive modulo reduction. No RNG is held across an await.
pub fn sample_delay(acquisition: u32) -> Result<crate::domain::retry::RetryDelay, PolicyError> {
    use rand::Rng;
    let ceiling = RetryBackoff::new(1_000_000, 300_000_000)?.ceiling_micros(acquisition)?;
    let floor = ceiling / 2 + ceiling % 2;
    selected_delay(
        acquisition,
        rand::thread_rng().gen_range(0..=ceiling - floor),
    )
}
