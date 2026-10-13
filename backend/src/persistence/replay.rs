//! Replay intent and receipts, independent of transport and storage drivers.
use super::PersistenceError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use uuid::Uuid;

/// Exact intent: strings are compared byte-for-byte; no implicit normalization.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayIntent {
    pub request_id: Uuid,
    pub event_id: Uuid,
    pub expected_revision: i64,
    pub expected_cycle: i32,
    pub reason: String,
    pub budget: i32,
    pub delay_ms: i64,
    /// Only bounded typed metadata is accepted, never arbitrary credentials/content.
    pub ticket: Option<Uuid>,
}
impl ReplayIntent {
    /// Validate structural bounds; authorization may impose a smaller budget.
    pub fn valid(&self) -> bool {
        !self.request_id.is_nil()
            && !self.event_id.is_nil()
            && self.expected_revision >= 0
            && self.expected_cycle > 0
            && (1..=100).contains(&self.budget)
            && (0..=86_400_000).contains(&self.delay_ms)
            && !self.reason.trim().is_empty()
            && self.reason.len() <= 256
            && self.reason.bytes().all(|b| (32..=126).contains(&b))
            && self.ticket.is_none_or(|id| !id.is_nil())
    }
}
/// Receipt means pending work was re-enabled, never broker/consumer success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayReceipt {
    pub request_id: Uuid,
    pub event_id: Uuid,
    pub cycle: i32,
    pub revision: i64,
    pub baseline: i32,
    pub budget: i32,
    pub decided_at: DateTime<Utc>,
    pub available_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutcome {
    Reenabled(ReplayReceipt),
    IntentConflict,
    StateConflict,
    Ineligible,
    CapacityExhausted,
}
/// Authorized intent, supplied by the use case after checking trusted identity.
#[derive(Clone)]
pub struct AuthorizedReplay {
    pub actor: String,
    pub scope: String,
    pub intent: ReplayIntent,
}
/// Implementation serializes request identity before locking the event; commits
/// receipt/audit and one new cycle atomically. No hidden retries. Identical intent
/// returns its receipt regardless of later event progress. Cancellation can hide
/// committed success; CommitUncertain requires authenticated observation/repetition.
pub trait OutboxReplay: Send + Sync {
    fn replay(
        &self,
        request: AuthorizedReplay,
    ) -> impl Future<Output = Result<ReplayOutcome, PersistenceError>> + Send;
    /// Negative observation does not prove an in-flight replay cannot commit.
    fn replay_outcome(
        &self,
        actor: &str,
        scope: &str,
        request_id: Uuid,
    ) -> impl Future<Output = Result<Option<ReplayReceipt>, PersistenceError>> + Send;
}
