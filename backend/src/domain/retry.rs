//! Durable retry data and invariants; no policy selection, storage or randomness.
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryError {
    InvalidHistory,
    InvalidDelay,
    InvalidTimestamp,
    RevisionLimit,
    InvalidIdentity,
}
impl fmt::Display for RetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid retry data: {self:?}")
    }
}
impl Error for RetryError {}

/// Persisted cycle history. Baseline never resets the lifetime acquisition counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryState {
    cycle: i32,
    baseline: i32,
    maximum: i32,
    revision: i64,
}
impl RetryState {
    pub fn restore(
        lifetime: u32,
        cycle: i32,
        baseline: i32,
        maximum: i32,
        revision: i64,
    ) -> Result<Self, RetryError> {
        if lifetime > i32::MAX as u32
            || cycle <= 0
            || baseline < 0
            || baseline as u32 > lifetime
            || maximum <= 0
            || revision < 0
        {
            return Err(RetryError::InvalidHistory);
        }
        Ok(Self {
            cycle,
            baseline,
            maximum,
            revision,
        })
    }
    pub fn initial(lifetime: u32) -> Result<Self, RetryError> {
        Self::restore(lifetime, 1, 0, 10, 0)
    }
    pub fn cycle(self) -> i32 {
        self.cycle
    }
    pub fn baseline(self) -> i32 {
        self.baseline
    }
    pub fn maximum(self) -> i32 {
        self.maximum
    }
    pub fn revision(self) -> i64 {
        self.revision
    }
    pub fn consumed(self, lifetime: u32) -> Result<u32, RetryError> {
        Self::restore(
            lifetime,
            self.cycle,
            self.baseline,
            self.maximum,
            self.revision,
        )?;
        lifetime
            .checked_sub(self.baseline as u32)
            .ok_or(RetryError::InvalidHistory)
    }
    pub fn can_acquire(self, lifetime: u32) -> Result<bool, RetryError> {
        let consumed = self.consumed(lifetime)?;
        Ok(lifetime < i32::MAX as u32 && consumed < self.maximum as u32)
    }
    pub fn next_revision(self) -> Result<i64, RetryError> {
        self.revision
            .checked_add(1)
            .ok_or(RetryError::RevisionLimit)
    }
}

/// Positive whole microseconds, at most one day. Caller selects backoff/jitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryDelay(i64);
impl RetryDelay {
    pub fn from_micros(micros: i64) -> Result<Self, RetryError> {
        if !(1..=86_400_000_000).contains(&micros) {
            return Err(RetryError::InvalidDelay);
        }
        Ok(Self(micros))
    }
    pub fn from_duration(duration: Duration) -> Result<Self, RetryError> {
        let micros = duration
            .num_microseconds()
            .ok_or(RetryError::InvalidDelay)?;
        if Duration::microseconds(micros) != duration {
            return Err(RetryError::InvalidDelay);
        }
        Self::from_micros(micros)
    }
    pub fn as_micros(self) -> i64 {
        self.0
    }
    pub fn available_at(self, now: DateTime<Utc>) -> Result<DateTime<Utc>, RetryError> {
        if !now.timestamp_subsec_nanos().is_multiple_of(1000) {
            return Err(RetryError::InvalidTimestamp);
        }
        let result = now
            .checked_add_signed(Duration::microseconds(self.0))
            .ok_or(RetryError::InvalidTimestamp)?;
        // Intersection of Chrono and finite PostgreSQL timestamp ranges.
        if now < postgres_minimum() || result < postgres_minimum() {
            return Err(RetryError::InvalidTimestamp);
        }
        Ok(result)
    }
}
fn postgres_minimum() -> DateTime<Utc> {
    chrono::NaiveDate::from_ymd_opt(-4713, 11, 24)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePhase {
    BeforeSend,
    BrokerResponse,
    Maintenance,
}
impl FailurePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeSend => "before_send",
            Self::BrokerResponse => "broker_response",
            Self::Maintenance => "maintenance",
        }
    }
}
/// Fixed allowlist: no free-form text, payload, driver errors or credential fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureEvidence {
    phase: FailurePhase,
    validator: bool,
    reference: Option<Uuid>,
}
impl FailureEvidence {
    pub fn new(
        phase: FailurePhase,
        validator: bool,
        reference: Option<Uuid>,
    ) -> Result<Self, RetryError> {
        if reference.is_some_and(|id| id.is_nil()) {
            return Err(RetryError::InvalidIdentity);
        }
        Ok(Self {
            phase,
            validator,
            reference,
        })
    }
    pub fn to_json(self) -> Value {
        let mut value = json!({"policy_version":"adr008-v1","phase":self.phase.as_str()});
        if self.validator {
            value["validator_version"] = json!("envelope-v1");
        }
        if let Some(reference) = self.reference {
            value["evidence_reference"] = json!(reference);
        }
        value
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDiagnostic {
    Unroutable,
    Rejected,
    SerializationRetryable,
}
impl RetryDiagnostic {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unroutable => "unroutable",
            Self::Rejected => "rejected",
            Self::SerializationRetryable => "serialization_retryable",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalReason {
    SerializationPermanent,
    InvalidStoredEnvelope,
    AcquisitionBudgetExhausted,
    StorageCounterExhausted,
}
impl TerminalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SerializationPermanent => "serialization_permanent",
            Self::InvalidStoredEnvelope => "invalid_stored_envelope",
            Self::AcquisitionBudgetExhausted => "acquisition_budget_exhausted",
            Self::StorageCounterExhausted => "storage_counter_exhausted",
        }
    }
}
