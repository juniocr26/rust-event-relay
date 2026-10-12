//! One acknowledged claim → original publication → ownership-protected transition.
//!
//! Cancellation can hide committed or accepted effects. There is no cleanup on
//! drop: abandoned claims recover through lease expiration. Database authority
//! does not fence external publication, even if acquisition returns after expiry.
use super::publisher::{EventPublisher, PublishError, PublishErrorKind};
use crate::{
    domain::delivery::{DeliveryLease, LeaseDuration, OwnershipToken},
    persistence::{
        AcquireRequest, OutboxAcquirer, OutboxCompleter, OutboxReleaser, OwnedEventKey,
        PersistenceError, TransitionOutcome,
    },
};

/// Diagnostic context without the envelope or diagnostic error sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryClaim {
    pub key: OwnedEventKey,
    pub lease: DeliveryLease,
    /// Committed acquisitions, not publication attempts.
    pub attempt_count: u32,
}

/// Cleanup is independent of the primary publication error.
#[derive(Debug)]
pub enum ReleaseOutcome {
    /// Acceptance may have occurred; expiration permits later recovery.
    /// RabbitMQ retires its uncertain channel; composition must replace it.
    LeaseRetained,
    /// Applied, OwnershipLost, or a classified error (including CommitUncertain).
    Attempted(Result<TransitionOutcome, PersistenceError>),
}

/// A completed delivery requires publication confirmation AND Applied completion.
/// Error kinds preserve uncertainty; no error implies an automatic retry.
/// Formatting contains neither payloads nor underlying diagnostic sources.
#[derive(Debug)]
pub enum DeliveryOutcome {
    NoEvent,
    /// No claim is exposed, even if an uncertain acquisition durably committed.
    AcquisitionFailed {
        token: OwnershipToken,
        duration: LeaseDuration,
        error: PersistenceError,
    },
    /// Broker acceptance is confirmed; completion may be Applied, OwnershipLost,
    /// failed, or uncertain. Only Ok(Applied) means durable completion acknowledged.
    Published {
        claim: DeliveryClaim,
        completion: Result<TransitionOutcome, PersistenceError>,
    },
    PublicationFailed {
        claim: DeliveryClaim,
        error: PublishError,
        release: ReleaseOutcome,
    },
}

/// Performs at most one acquisition and one publication, with no internal retry.
///
/// Unavailable/Serialization occur before send; Unroutable/Rejected are definitive
/// broker failures. Each triggers one release. Uncertain retains the lease.
/// Completion is attempted only after confirmed acceptance; failed/uncertain
/// completion never triggers release or another send. Recovery may duplicate the
/// original event ID. No application-clock ownership check is authoritative.
pub async fn deliver_once<R, P>(
    repository: &R,
    publisher: &P,
    request: AcquireRequest,
) -> DeliveryOutcome
where
    R: OutboxAcquirer + OutboxCompleter + OutboxReleaser,
    P: EventPublisher + Sync,
{
    let token = request.token();
    let duration = request.duration();
    let owned = match repository.acquire(request).await {
        Ok(Some(owned)) => owned,
        Ok(None) => return DeliveryOutcome::NoEvent,
        Err(error) => {
            return DeliveryOutcome::AcquisitionFailed {
                token,
                duration,
                error,
            };
        }
    };
    let claim = DeliveryClaim {
        key: owned.key(),
        lease: owned.lease(),
        attempt_count: owned.snapshot().attempt_count(),
    };
    match publisher.publish(owned.snapshot().event()).await {
        Ok(()) => DeliveryOutcome::Published {
            claim,
            completion: repository.complete(claim.key).await,
        },
        Err(error) => {
            let release = match error.kind() {
                PublishErrorKind::Uncertain => ReleaseOutcome::LeaseRetained,
                PublishErrorKind::Unavailable
                | PublishErrorKind::Serialization
                | PublishErrorKind::Unroutable
                | PublishErrorKind::Rejected => {
                    ReleaseOutcome::Attempted(repository.release(claim.key).await)
                }
            };
            DeliveryOutcome::PublicationFailed {
                claim,
                error,
                release,
            }
        }
    }
}
