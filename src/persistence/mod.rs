//! Application-facing, read-only outbox persistence boundary.
//!
//! Returned pending events are snapshots, not claims. Producer transactions and
//! relay lifecycle mutations require separate ownership/recovery contracts later.
mod error;
mod model;

pub use error::{PersistenceError, PersistenceErrorKind};
pub use model::{BatchSize, EligibleRead, InvalidBatchSize, PendingOutboxEvent};

use std::future::Future;

/// Bounded observation of eligible pending events, independent of a storage driver.
///
/// Implementations must return at most the requested limit, with each event
/// pending and available at the cutoff in one consistent read snapshot. Reads
/// must not mutate lifecycle state. No selection or business ordering is promised.
///
/// Repeating a read has no side effects but may return the same events or a
/// changed snapshot. Returning an event confers no ownership: other readers may
/// receive it, and concurrent writes may invalidate its metadata immediately.
///
/// Failures use `Unavailable`, `InvalidStoredData` or `OperationFailed`; malformed
/// selected data must fail the read rather than being silently skipped or fixed.
/// No producer atomicity, claim recovery or exactly-once delivery is guaranteed.
/// Native futures are Send for generic callers on multithreaded executors; the
/// trait deliberately uses static dispatch rather than a driver-facing dyn API.
pub trait OutboxReader: Send + Sync {
    /// Reads pending snapshots available at or before `request.eligible_at()`.
    ///
    /// A successful empty batch means no eligible rows in this read snapshot,
    /// not that the durable backlog is empty or that it will stay empty.
    fn read_eligible(
        &self,
        request: EligibleRead,
    ) -> impl Future<Output = Result<Vec<PendingOutboxEvent>, PersistenceError>> + Send;
}
