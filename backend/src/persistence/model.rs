use std::{error::Error, fmt, num::NonZeroUsize};

use chrono::{DateTime, Utc};

use crate::domain::event::EventEnvelope;

/// Positive upper bound on returned work, not a concurrency or byte-size limit.
/// Operational caps belong to future caller configuration; no arbitrary cap is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchSize(NonZeroUsize);

impl BatchSize {
    /// Rejects zero rather than interpreting it as an unbounded read.
    pub fn new(value: usize) -> Result<Self, InvalidBatchSize> {
        NonZeroUsize::new(value).map(Self).ok_or(InvalidBatchSize)
    }

    /// Maximum number of events the caller requests.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

/// A zero-sized batch would not express useful bounded work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBatchSize;

impl fmt::Display for InvalidBatchSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("batch size must be greater than zero")
    }
}

impl Error for InvalidBatchSize {}

/// Explicit UTC eligibility cutoff and positive maximum result count.
/// This request neither selects a retry policy nor reserves returned work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EligibleRead {
    eligible_at: DateTime<Utc>,
    limit: BatchSize,
}

impl EligibleRead {
    /// Constructs a deterministic request without consulting the system clock.
    pub fn new(eligible_at: DateTime<Utc>, limit: BatchSize) -> Self {
        Self { eligible_at, limit }
    }

    /// Latest availability instant eligible for this read.
    pub fn eligible_at(self) -> DateTime<Utc> {
        self.eligible_at
    }

    /// Maximum number of returned snapshots.
    pub fn limit(self) -> BatchSize {
        self.limit
    }
}

/// A pending-event snapshot containing only metadata needed for eligibility/attempts.
/// This is neither a complete database row nor a durable claim or delivery token.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingOutboxEvent {
    event: EventEnvelope,
    attempt_count: u32,
    available_at: DateTime<Utc>,
}

impl PendingOutboxEvent {
    /// Wraps an already validated envelope without generating new identity/time.
    /// Adapters must verify pending state and validate stored numeric/time values
    /// before construction; storage decoding belongs outside this model.
    pub fn new(event: EventEnvelope, attempt_count: u32, available_at: DateTime<Utc>) -> Self {
        Self {
            event,
            attempt_count,
            available_at,
        }
    }

    /// Canonical event; relay metadata is not added to the envelope.
    pub fn event(&self) -> &EventEnvelope {
        &self.event
    }

    /// Observed count of committed acquisitions (historical values preserved),
    /// not actual broker sends or permission to increment it.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Earliest eligibility time; it may already be stale after the read.
    pub fn available_at(&self) -> DateTime<Utc> {
        self.available_at
    }
}
