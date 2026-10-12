//! Focused retry transitions. No hidden retry or publication authority is implied.
use super::{OwnedEventKey, PersistenceError};
use crate::domain::retry::{
    FailureEvidence, RetryDelay, RetryDiagnostic, RetryError, RetryState, TerminalReason,
};
use chrono::{DateTime, Utc};
use std::future::Future;
use uuid::Uuid;

/// Expected cycle/revision plus ownership; a snapshot alone never authorizes writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryOwner {
    key: OwnedEventKey,
    cycle: i32,
    revision: i64,
}
impl RetryOwner {
    pub fn new(key: OwnedEventKey, state: RetryState) -> Result<Self, RetryError> {
        if key.event_id().is_nil() {
            return Err(RetryError::InvalidIdentity);
        }
        Ok(Self {
            key,
            cycle: state.cycle(),
            revision: state.revision(),
        })
    }
    pub fn key(self) -> OwnedEventKey {
        self.key
    }
    pub fn cycle(self) -> i32 {
        self.cycle
    }
    pub fn revision(self) -> i64 {
        self.revision
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ScheduleRetry {
    pub owner: RetryOwner,
    pub delay: RetryDelay,
    pub diagnostic: RetryDiagnostic,
    pub evidence: FailureEvidence,
}
#[derive(Debug, Clone, Copy)]
pub struct PreserveFailure {
    pub owner: RetryOwner,
    pub reason: TerminalReason,
    pub evidence: FailureEvidence,
}

/// Exact raw-row maintenance authority, without inventing a valid owned envelope.
/// One event per invocation; caller must supply an independently observed revision.
#[derive(Debug, Clone, Copy)]
pub struct ExhaustedEvent {
    id: Uuid,
    cycle: i32,
    revision: i64,
}
impl ExhaustedEvent {
    pub fn new(id: Uuid, cycle: i32, revision: i64) -> Result<Self, RetryError> {
        if id.is_nil() {
            return Err(RetryError::InvalidIdentity);
        }
        if cycle <= 0 || revision < 0 {
            return Err(RetryError::InvalidHistory);
        }
        Ok(Self {
            id,
            cycle,
            revision,
        })
    }
    pub fn id(self) -> Uuid {
        self.id
    }
    pub fn cycle(self) -> i32 {
        self.cycle
    }
    pub fn revision(self) -> i64 {
        self.revision
    }
}
/// Conflict includes missing/stale identity, cycle/revision, expired authority,
/// exhausted scheduling budget, live maintenance owner or non-exhausted row.
/// Repeats return Conflict, never inferred idempotent success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryTransition<T> {
    Applied(T),
    Conflict,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledRetry {
    pub available_at: DateTime<Utc>,
    pub revision: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreservedFailure {
    pub terminal_at: DateTime<Utc>,
    pub reason: TerminalReason,
    pub revision: i64,
}

/// Lock exact row, sample database time, require live owner and cycle/revision,
/// persist selected delay/evidence and clear ownership atomically. No randomness.
/// Only definitive contextual failure authorizes calling this port; unknown broker
/// acceptance never does. Exhausted cycles conflict, including the final attempt.
pub trait OutboxRetryScheduler: Send + Sync {
    fn schedule_retry(
        &self,
        request: ScheduleRetry,
    ) -> impl Future<Output = Result<RetryTransition<ScheduledRetry>, PersistenceError>> + Send;
}
/// Persists an application-authorized terminal decision, not broker acceptance.
/// Requires live ownership and expected cycle/revision, retains the original row.
pub trait OutboxFailurePreserver: Send + Sync {
    fn preserve_failure(
        &self,
        request: PreserveFailure,
    ) -> impl Future<Output = Result<RetryTransition<PreservedFailure>, PersistenceError>> + Send;
}
/// Due pending exhausted metadata only, absent/expired lease, expected cycle/revision.
/// Exact-ID SKIP LOCKED; live owners and locked rows conflict. Never validates or
/// repairs envelopes. Storage exhaustion takes precedence over cycle exhaustion.
/// All three ports commit before Applied. Known rejection rolls back; lost COMMIT
/// acknowledgement is CommitUncertain. Cancellation can suppress a durable result;
/// dropping a future does not prove immediate rollback. No blind repeat is safe.
pub trait OutboxExhaustedMaintainer: Send + Sync {
    fn preserve_exhausted(
        &self,
        request: ExhaustedEvent,
    ) -> impl Future<Output = Result<RetryTransition<PreservedFailure>, PersistenceError>> + Send;
}
