//! Closed operational observations, independent of an exporter or domain model.
use std::{
    future::Future,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum Operation {
    Acquisition,
    Publication,
    Completion,
    Schedule,
    Terminal,
    Release,
    Maintenance,
    Processing,
    Quarantine,
    Ack,
    Connect,
    Close,
    Shutdown,
    Http,
    Delivery,
    Replay,
}
impl Operation {
    pub const ALL: [Self; 16] = [
        Self::Acquisition,
        Self::Publication,
        Self::Completion,
        Self::Schedule,
        Self::Terminal,
        Self::Release,
        Self::Maintenance,
        Self::Processing,
        Self::Quarantine,
        Self::Ack,
        Self::Connect,
        Self::Close,
        Self::Shutdown,
        Self::Http,
        Self::Delivery,
        Self::Replay,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Acquisition => "acquisition",
            Self::Publication => "publication",
            Self::Completion => "completion",
            Self::Schedule => "schedule",
            Self::Terminal => "terminal",
            Self::Release => "release",
            Self::Maintenance => "maintenance",
            Self::Processing => "processing",
            Self::Quarantine => "quarantine",
            Self::Ack => "ack_send",
            Self::Connect => "connect",
            Self::Close => "close",
            Self::Shutdown => "shutdown",
            Self::Http => "http",
            Self::Delivery => "delivery",
            Self::Replay => "replay",
        }
    }
}
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum Outcome {
    Acquired,
    Empty,
    Confirmed,
    Applied,
    Duplicate,
    IdentityConflict,
    Invalid,
    Failed,
    Unavailable,
    CommitUncertain,
    Uncertain,
    OwnershipLost,
    Conflict,
    Rejected,
    Unroutable,
    Serialization,
    Blocked,
    Observed,
    End,
    Cancelled,
    Completed,
    Deadline,
    Reenabled,
    Denied,
    Saturated,
}
impl Outcome {
    pub const ALL: [Self; 25] = [
        Self::Acquired,
        Self::Empty,
        Self::Confirmed,
        Self::Applied,
        Self::Duplicate,
        Self::IdentityConflict,
        Self::Invalid,
        Self::Failed,
        Self::Unavailable,
        Self::CommitUncertain,
        Self::Uncertain,
        Self::OwnershipLost,
        Self::Conflict,
        Self::Rejected,
        Self::Unroutable,
        Self::Serialization,
        Self::Blocked,
        Self::Observed,
        Self::End,
        Self::Cancelled,
        Self::Completed,
        Self::Deadline,
        Self::Reenabled,
        Self::Denied,
        Self::Saturated,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Acquired => "acquired",
            Self::Empty => "empty",
            Self::Confirmed => "confirmed",
            Self::Applied => "applied",
            Self::Duplicate => "duplicate",
            Self::IdentityConflict => "identity_conflict",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
            Self::CommitUncertain => "commit_uncertain",
            Self::Uncertain => "uncertain",
            Self::OwnershipLost => "ownership_lost",
            Self::Conflict => "conflict",
            Self::Rejected => "rejected",
            Self::Unroutable => "unroutable",
            Self::Serialization => "serialization",
            Self::Blocked => "blocked",
            Self::Observed => "observed",
            Self::End => "end",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::Deadline => "deadline",
            Self::Reenabled => "reenabled",
            Self::Denied => "denied",
            Self::Saturated => "saturated",
        }
    }
}
/// Implementations must be bounded, synchronous and perform no external I/O.
pub trait Observer: Sync {
    fn broker_readiness(&self, _usable: bool) {}
    fn event(&self, operation: Operation, outcome: Outcome);
    fn enter(&self, operation: Operation);
    fn exit(&self, operation: Operation, elapsed: Duration);
}
pub struct Noop;
impl Observer for Noop {
    fn event(&self, _: Operation, _: Outcome) {}
    fn enter(&self, _: Operation) {}
    fn exit(&self, _: Operation, _: Duration) {}
}
/// Drop records cancellation and releases the in-flight observation, never business ownership.
pub struct Observation<'a, O: Observer> {
    observer: &'a O,
    operation: Operation,
    started: Instant,
    finished: bool,
}
impl<'a, O: Observer> Observation<'a, O> {
    pub fn new(observer: &'a O, operation: Operation) -> Self {
        observer.enter(operation);
        Self {
            observer,
            operation,
            started: Instant::now(),
            finished: false,
        }
    }
    pub fn finish(mut self, outcome: Outcome) {
        self.observer.event(self.operation, outcome);
        self.finished = true;
    }
}
impl<O: Observer> Drop for Observation<'_, O> {
    fn drop(&mut self) {
        if !self.finished {
            self.observer.event(self.operation, Outcome::Cancelled);
        }
        self.observer.exit(self.operation, self.started.elapsed());
    }
}
pub async fn observe<T, O: Observer>(
    observer: &O,
    operation: Operation,
    future: impl Future<Output = T>,
    classify: impl FnOnce(&T) -> Outcome,
) -> T {
    let guard = Observation::new(observer, operation);
    let result = future.await;
    guard.finish(classify(&result));
    result
}
pub fn persistence(error: &crate::persistence::PersistenceError) -> Outcome {
    use crate::persistence::PersistenceErrorKind::*;
    match error.kind() {
        CommitUncertain => Outcome::CommitUncertain,
        Unavailable => Outcome::Unavailable,
        InvalidStoredData => Outcome::Invalid,
        OperationFailed => Outcome::Failed,
    }
}
pub fn publication(result: &Result<(), super::publisher::PublishError>) -> Outcome {
    use super::publisher::PublishErrorKind::*;
    match result {
        Ok(()) => Outcome::Confirmed,
        Err(e) => match e.kind() {
            Uncertain => Outcome::Uncertain,
            Unavailable => Outcome::Unavailable,
            Serialization => Outcome::Serialization,
            Rejected | RejectedBeforeSend => Outcome::Rejected,
            Unroutable => Outcome::Unroutable,
        },
    }
}
pub fn transition(
    result: &Result<crate::persistence::TransitionOutcome, crate::persistence::PersistenceError>,
) -> Outcome {
    match result {
        Ok(crate::persistence::TransitionOutcome::Applied) => Outcome::Applied,
        Ok(_) => Outcome::OwnershipLost,
        Err(e) => persistence(e),
    }
}
pub fn retry<T>(
    result: &Result<crate::persistence::RetryTransition<T>, crate::persistence::PersistenceError>,
) -> Outcome {
    match result {
        Ok(crate::persistence::RetryTransition::Applied(_)) => Outcome::Applied,
        Ok(_) => Outcome::Conflict,
        Err(e) => persistence(e),
    }
}
