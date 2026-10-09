//! Future atomic database operations; no production implementation in Milestone 2.2.
use super::{PendingOutboxEvent, PersistenceError};
use crate::domain::delivery::{DeliveryError, DeliveryLease, LeaseDuration, OwnershipToken};
use std::future::Future;
use uuid::Uuid;

/// One acquisition invocation with a fresh token, retained across unknown outcomes.
/// Never retry acquisition using the same token, including after cancellation.
#[derive(Debug)]
pub struct AcquireRequest {
    token: OwnershipToken,
    duration: LeaseDuration,
}
impl AcquireRequest {
    pub fn new(duration: LeaseDuration) -> Self {
        Self {
            token: OwnershipToken::fresh(),
            duration,
        }
    }
    pub fn token(&self) -> OwnershipToken {
        self.token
    }
    pub fn duration(&self) -> LeaseDuration {
        self.duration
    }
}

/// Event identity plus expected ownership identity; ID alone never authorizes writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedEventKey {
    event_id: Uuid,
    token: OwnershipToken,
}
impl OwnedEventKey {
    pub fn new(event_id: Uuid, token: OwnershipToken) -> Self {
        Self { event_id, token }
    }
    pub fn event_id(self) -> Uuid {
        self.event_id
    }
    pub fn token(self) -> OwnershipToken {
        self.token
    }
}

/// Confirmed committed acquisition, preserving the observed envelope identity.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedOutboxEvent {
    snapshot: PendingOutboxEvent,
    lease: DeliveryLease,
}
impl OwnedOutboxEvent {
    pub fn new(snapshot: PendingOutboxEvent, lease: DeliveryLease) -> Result<Self, DeliveryError> {
        if snapshot.attempt_count() == 0
            || snapshot.attempt_count() > crate::domain::delivery::DeliveryState::MAX_ATTEMPTS
        {
            return Err(DeliveryError::InvalidState);
        }
        if snapshot.available_at() > lease.acquired_at() {
            return Err(DeliveryError::InvalidTimestamp);
        }
        Ok(Self { snapshot, lease })
    }
    pub fn snapshot(&self) -> &PendingOutboxEvent {
        &self.snapshot
    }
    pub fn lease(&self) -> DeliveryLease {
        self.lease
    }
    pub fn key(&self) -> OwnedEventKey {
        OwnedEventKey::new(self.snapshot.event().id(), self.lease.token())
    }
}

/// Atomic predicate result, distinct from infrastructure failure.
/// Repeated calls after success return OwnershipLost, never presumed success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionOutcome {
    Applied,
    OwnershipLost,
}

/// Atomically selects at most one due pending row without active ownership or an
/// exhausted counter. Locks, samples database time, increments attempts, assigns
/// request token and lease, then commits before returning Some. None means no
/// acquirable row, not an empty backlog. No network publication inside transaction.
/// All changes roll back on known failure; unknown commit is CommitUncertain.
/// Dropped futures may leave committed leases; expiry is the recovery path.
pub trait OutboxAcquirer: Send + Sync {
    fn acquire(
        &self,
        request: AcquireRequest,
    ) -> impl Future<Output = Result<Option<OwnedOutboxEvent>, PersistenceError>> + Send;
}
/// Atomic pending + event ID + token + unexpired lease predicate, evaluated using
/// database time after lock acquisition. Applied sets processed/processed_at and
/// clears all ownership fields together. No match is OwnershipLost. Only a future
/// use case's confirmed publication authorizes this operation. CommitUncertain
/// requires reconciliation; it is not known failure or idempotent success.
pub trait OutboxCompleter: Send + Sync {
    fn complete(
        &self,
        owner: OwnedEventKey,
    ) -> impl Future<Output = Result<TransitionOutcome, PersistenceError>> + Send;
}
/// Same authority/time predicate as completion. Applied clears ownership together,
/// retaining pending status, attempts, availability and envelope. No retry policy.
/// No match is OwnershipLost; cancellation/unknown COMMIT may have applied.
pub trait OutboxReleaser: Send + Sync {
    fn release(
        &self,
        owner: OwnedEventKey,
    ) -> impl Future<Output = Result<TransitionOutcome, PersistenceError>> + Send;
}
