//! Atomic durable ownership operations, independent of storage drivers.
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
    event_id: Option<Uuid>,
}
impl AcquireRequest {
    pub fn new(duration: LeaseDuration) -> Self {
        Self {
            token: OwnershipToken::fresh(),
            duration,
            event_id: None,
        }
    }
    /// Focus selection on a discovered identity; this conveys no claim or revision
    /// authority. All current acquisition predicates still apply under the lock.
    pub fn targeting(mut self, id: Uuid) -> Result<Self, DeliveryError> {
        if id.is_nil() {
            return Err(DeliveryError::InvalidState);
        }
        self.event_id = Some(id);
        Ok(self)
    }
    pub fn event_id(&self) -> Option<Uuid> {
        self.event_id
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
    retry_state: crate::domain::retry::RetryState,
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
        let retry_state = crate::domain::retry::RetryState::initial(snapshot.attempt_count())
            .map_err(|_| DeliveryError::InvalidState)?;
        Ok(Self {
            snapshot,
            lease,
            retry_state,
        })
    }
    pub fn with_retry_state(
        mut self,
        state: crate::domain::retry::RetryState,
    ) -> Result<Self, crate::domain::retry::RetryError> {
        state.consumed(self.snapshot.attempt_count())?;
        self.retry_state = state;
        Ok(self)
    }
    pub fn retry_state(&self) -> crate::domain::retry::RetryState {
        self.retry_state
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
