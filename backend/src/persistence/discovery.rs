//! Bounded raw-candidate observations; no envelope or lease is fabricated.
use super::{ExhaustedEvent, PersistenceError, PreservedFailure, RetryTransition};
use crate::domain::{event::EventError, retry::TerminalReason};
use std::future::Future;
use uuid::Uuid;

/// One pending row per pass in UUID order. None starts/restarts a traversal.
/// Empty results end this suffix only. Progress is process-local, not durable.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscoveryCursor {
    pub after: Option<Uuid>,
    pub through: Option<Uuid>,
}

/// Fixed sanitized evidence. Decoder failures never authorize isolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateEvidence {
    Valid,
    LiveOwner,
    NotDue,
    RepairRequired,
    UnsupportedRepresentation,
    InvalidEnvelope(EventError),
    Exhausted(TerminalReason),
}
impl CandidateEvidence {
    pub fn terminal_reason(&self) -> Option<TerminalReason> {
        match self {
            Self::InvalidEnvelope(_) => Some(TerminalReason::InvalidStoredEnvelope),
            Self::Exhausted(reason) => Some(*reason),
            _ => None,
        }
    }
}

/// Observation only. Revalidation under a fresh row lock is mandatory.
#[derive(Debug, Clone)]
pub struct MaintenanceCandidate {
    pub expected: ExhaustedEvent,
    pub evidence: CandidateEvidence,
}
#[derive(Debug, Clone)]
pub struct DiscoveryObservation {
    pub cursor: DiscoveryCursor,
    pub evidence: CandidateEvidence,
    pub candidate: Option<MaintenanceCandidate>,
}

/// At most one row/payload materialized; no locks or writes during discovery.
/// Every pending row is visited, including not-due and live-owned rows, so filters
/// cannot hide invalid authority metadata. Concurrent scanners may observe the
/// same row. Insertions/changes behind the cursor are revisited after wrap.
pub trait OutboxCandidateDiscovery: Send + Sync {
    fn discover_candidate(
        &self,
        cursor: DiscoveryCursor,
    ) -> impl Future<Output = Result<Option<DiscoveryObservation>, PersistenceError>> + Send;
}

/// Exact-row SKIP LOCKED, post-lock database clock, expected cycle/revision,
/// checked authority and freshly revalidated evidence. No hidden repeats.
/// CommitUncertain/cancellation may hide durable isolation; observe anew.
pub trait OutboxCandidateIsolator: Send + Sync {
    fn isolate_candidate(
        &self,
        candidate: MaintenanceCandidate,
    ) -> impl Future<Output = Result<RetryTransition<PreservedFailure>, PersistenceError>> + Send;
}
