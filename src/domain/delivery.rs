//! Durable delivery metadata, independent of database and transport drivers.
use chrono::{DateTime, Duration, Utc};
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryError {
    InvalidDuration,
    InvalidTimestamp,
    InvalidToken,
    InvalidState,
    NotEligible,
    OwnershipLost,
    AttemptLimit,
}
impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid delivery operation: {self:?}")
    }
}
impl Error for DeliveryError {}

/// Positive whole microseconds, matching durable timestamp precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseDuration(i64);
impl LeaseDuration {
    pub fn from_micros(value: i64) -> Result<Self, DeliveryError> {
        if value <= 0 {
            return Err(DeliveryError::InvalidDuration);
        }
        Ok(Self(value))
    }
    pub fn as_micros(self) -> i64 {
        self.0
    }
}

/// Ownership identity; never substitute an event ID or reuse an acquisition token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipToken(Uuid);
impl OwnershipToken {
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }
    pub fn restore(value: Uuid) -> Result<Self, DeliveryError> {
        if value.is_nil() {
            return Err(DeliveryError::InvalidToken);
        }
        Ok(Self(value))
    }
    pub fn get(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryLease {
    token: OwnershipToken,
    acquired_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}
fn microsecond_time(time: DateTime<Utc>) -> bool {
    time.timestamp_subsec_nanos().is_multiple_of(1000)
}
impl DeliveryLease {
    pub fn new(
        token: OwnershipToken,
        acquired_at: DateTime<Utc>,
        duration: LeaseDuration,
    ) -> Result<Self, DeliveryError> {
        let expires_at = acquired_at
            .checked_add_signed(Duration::microseconds(duration.as_micros()))
            .ok_or(DeliveryError::InvalidTimestamp)?;
        Self::restore(token, acquired_at, expires_at)
    }
    pub fn restore(
        token: OwnershipToken,
        acquired_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, DeliveryError> {
        if !microsecond_time(acquired_at)
            || !microsecond_time(expires_at)
            || expires_at <= acquired_at
        {
            return Err(DeliveryError::InvalidTimestamp);
        }
        Ok(Self {
            token,
            acquired_at,
            expires_at,
        })
    }
    pub fn token(self) -> OwnershipToken {
        self.token
    }
    pub fn acquired_at(self) -> DateTime<Utc> {
        self.acquired_at
    }
    pub fn expires_at(self) -> DateTime<Utc> {
        self.expires_at
    }
    pub fn is_active_at(self, now: DateTime<Utc>) -> bool {
        self.acquired_at <= now && now < self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Pending,
    Processed,
    DeadLetter,
}

/// Row-shape invariants and pure transition rules. This model performs no I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryState {
    status: DeliveryStatus,
    attempt_count: u32,
    available_at: DateTime<Utc>,
    processed_at: Option<DateTime<Utc>>,
    lease: Option<DeliveryLease>,
}
impl DeliveryState {
    pub const MAX_ATTEMPTS: u32 = i32::MAX as u32;
    pub fn restore(
        status: DeliveryStatus,
        attempt_count: u32,
        available_at: DateTime<Utc>,
        processed_at: Option<DateTime<Utc>>,
        lease: Option<DeliveryLease>,
    ) -> Result<Self, DeliveryError> {
        if attempt_count > Self::MAX_ATTEMPTS {
            return Err(DeliveryError::AttemptLimit);
        }
        if (status == DeliveryStatus::Processed) != processed_at.is_some()
            || (lease.is_some() && (status != DeliveryStatus::Pending || attempt_count == 0))
            || lease.is_some_and(|lease| available_at > lease.acquired_at())
        {
            return Err(DeliveryError::InvalidState);
        }
        Ok(Self {
            status,
            attempt_count,
            available_at,
            processed_at,
            lease,
        })
    }
    pub fn status(&self) -> DeliveryStatus {
        self.status
    }
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
    pub fn available_at(&self) -> DateTime<Utc> {
        self.available_at
    }
    pub fn processed_at(&self) -> Option<DateTime<Utc>> {
        self.processed_at
    }
    pub fn lease(&self) -> Option<DeliveryLease> {
        self.lease
    }
    /// Checks eligibility and replaces expired authority; errors leave self unchanged.
    pub fn acquire(
        &mut self,
        now: DateTime<Utc>,
        token: OwnershipToken,
        duration: LeaseDuration,
    ) -> Result<DeliveryLease, DeliveryError> {
        if self.status != DeliveryStatus::Pending
            || self.available_at > now
            || self.lease.is_some_and(|lease| now < lease.expires_at())
        {
            return Err(DeliveryError::NotEligible);
        }
        if self.lease.is_some_and(|lease| lease.token() == token) {
            return Err(DeliveryError::InvalidToken);
        }
        if self.attempt_count == Self::MAX_ATTEMPTS {
            return Err(DeliveryError::AttemptLimit);
        }
        let lease = DeliveryLease::new(token, now, duration)?;
        self.attempt_count += 1;
        self.lease = Some(lease);
        Ok(lease)
    }
    fn verify_owner(&self, token: OwnershipToken, now: DateTime<Utc>) -> Result<(), DeliveryError> {
        if self.status != DeliveryStatus::Pending
            || !self
                .lease
                .is_some_and(|lease| lease.token() == token && lease.is_active_at(now))
        {
            return Err(DeliveryError::OwnershipLost);
        }
        Ok(())
    }
    /// Future use case may call only after confirmed broker acceptance.
    pub fn complete(
        &mut self,
        token: OwnershipToken,
        now: DateTime<Utc>,
    ) -> Result<(), DeliveryError> {
        self.verify_owner(token, now)?;
        if !microsecond_time(now) {
            return Err(DeliveryError::InvalidTimestamp);
        }
        self.status = DeliveryStatus::Processed;
        self.processed_at = Some(now);
        self.lease = None;
        Ok(())
    }
    /// Clears authority without selecting a retry schedule or changing availability.
    pub fn release(
        &mut self,
        token: OwnershipToken,
        now: DateTime<Utc>,
    ) -> Result<(), DeliveryError> {
        self.verify_owner(token, now)?;
        self.lease = None;
        Ok(())
    }
}
