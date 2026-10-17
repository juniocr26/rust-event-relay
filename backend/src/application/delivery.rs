//! One acknowledged claim → original publication → ownership-protected transition.
//!
//! Cancellation can hide committed or accepted effects. There is no cleanup on
//! drop: abandoned claims recover through lease expiration. Database authority
//! does not fence external publication, even if acquisition returns after expiry.
use super::observation::{self, Noop, Observer, Operation as Op, Outcome as Obs, observe};
use super::publisher::{EventPublisher, PublishError, PublishErrorKind};
use crate::{
    domain::delivery::{DeliveryLease, LeaseDuration, OwnershipToken},
    persistence::{
        AcquireRequest, OutboxAcquirer, OutboxCompleter, OutboxFailurePreserver, OutboxReleaser,
        OutboxRetryScheduler, OwnedEventKey, PersistenceError, TransitionOutcome,
    },
};

/// Diagnostic context without the envelope or diagnostic error sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryClaim {
    pub key: OwnedEventKey,
    pub lease: DeliveryLease,
    /// Committed acquisitions, not publication attempts.
    pub attempt_count: u32,
    pub retry: crate::domain::retry::RetryState,
}

/// Event handling remains independent of the primary publication error.
#[derive(Debug)]
pub enum FailureHandling {
    /// Unknown acceptance, infrastructure failure, or insufficient encoding context.
    LeaseRetained,
    /// Before-send infrastructure unavailability only; no event retry schedule bypass.
    Released(Result<TransitionOutcome, PersistenceError>),
    Scheduled(
        Result<
            crate::persistence::RetryTransition<crate::persistence::ScheduledRetry>,
            PersistenceError,
        >,
    ),
    Terminal(
        Result<
            crate::persistence::RetryTransition<crate::persistence::PreservedFailure>,
            PersistenceError,
        >,
    ),
    /// Invalid policy calculation prevents any persistence transition.
    PolicyFailed(super::retry_policy::PolicyError),
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
        handling: FailureHandling,
    },
}

/// One acknowledged acquisition, at most one original publication and one transition.
/// Uncertain acceptance and uncontextualized serialization retain ownership.
/// Known before-send unavailability releases for infrastructure recovery only.
/// Definitive routed/rejected failures schedule within budget; definitive final
/// failures preserve terminal history. Completion failure has no compensation.
/// Cancellation at any await may hide effects; there is no cleanup on drop.
pub async fn deliver_once<R, P>(
    repository: &R,
    publisher: &P,
    request: AcquireRequest,
) -> DeliveryOutcome
where
    R: OutboxAcquirer
        + OutboxCompleter
        + OutboxRetryScheduler
        + OutboxFailurePreserver
        + OutboxReleaser,
    P: EventPublisher + Sync,
{
    deliver_once_with_delay(
        repository,
        publisher,
        request,
        super::retry_policy::sample_delay,
    )
    .await
}

/// Deterministic sampling seam. Called exactly once only for a scheduling decision.
pub async fn deliver_once_with_delay<R, P>(
    repository: &R,
    publisher: &P,
    request: AcquireRequest,
    select_delay: impl FnOnce(
        u32,
    ) -> Result<
        crate::domain::retry::RetryDelay,
        super::retry_policy::PolicyError,
    > + Send,
) -> DeliveryOutcome
where
    R: OutboxAcquirer
        + OutboxCompleter
        + OutboxRetryScheduler
        + OutboxFailurePreserver
        + OutboxReleaser,
    P: EventPublisher + Sync,
{
    deliver_once_observed(repository, publisher, request, select_delay, &Noop).await
}

pub async fn deliver_once_observed<R, P, O: Observer>(
    repository: &R,
    publisher: &P,
    request: AcquireRequest,
    select_delay: impl FnOnce(
        u32,
    ) -> Result<
        crate::domain::retry::RetryDelay,
        super::retry_policy::PolicyError,
    > + Send,
    observer: &O,
) -> DeliveryOutcome
where
    R: OutboxAcquirer
        + OutboxCompleter
        + OutboxRetryScheduler
        + OutboxFailurePreserver
        + OutboxReleaser,
    P: EventPublisher + Sync,
{
    let token = request.token();
    let duration = request.duration();
    let owned = match observe(
        observer,
        Op::Acquisition,
        repository.acquire(request),
        |r| match r {
            Ok(Some(_)) => Obs::Acquired,
            Ok(None) => Obs::Empty,
            Err(e) => observation::persistence(e),
        },
    )
    .await
    {
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
        retry: owned.retry_state(),
    };
    match observe(
        observer,
        Op::Publication,
        publisher.publish(owned.snapshot().event()),
        observation::publication,
    )
    .await
    {
        Ok(()) => DeliveryOutcome::Published {
            claim,
            completion: observe(
                observer,
                Op::Completion,
                repository.complete(claim.key),
                observation::transition,
            )
            .await,
        },
        Err(error) => {
            use crate::{domain::retry::*, persistence::*};
            let owner = RetryOwner::new(claim.key, claim.retry).expect("validated owned identity");
            let phase = match error.kind() {
                PublishErrorKind::Rejected | PublishErrorKind::Unroutable => {
                    FailurePhase::BrokerResponse
                }
                _ => FailurePhase::BeforeSend,
            };
            let evidence = FailureEvidence::new(phase, false, None).expect("fixed evidence");
            // Uncertainty takes precedence over exhaustion: even a final owner
            // must retain its lease when publication may have been accepted.
            let handling = if error.kind() == PublishErrorKind::Uncertain {
                FailureHandling::LeaseRetained
            } else if let Some(evidence) = error.permanent_evidence() {
                FailureHandling::Terminal(
                    observe(
                        observer,
                        Op::Terminal,
                        repository.preserve_failure(PreserveFailure {
                            owner,
                            reason: TerminalReason::SerializationPermanent,
                            evidence,
                        }),
                        observation::retry,
                    )
                    .await,
                )
            } else if !claim
                .retry
                .can_acquire(claim.attempt_count)
                .expect("validated history")
            {
                let reason = if claim.attempt_count == i32::MAX as u32 {
                    TerminalReason::StorageCounterExhausted
                } else {
                    TerminalReason::AcquisitionBudgetExhausted
                };
                FailureHandling::Terminal(
                    observe(
                        observer,
                        Op::Terminal,
                        repository.preserve_failure(PreserveFailure {
                            owner,
                            reason,
                            evidence,
                        }),
                        observation::retry,
                    )
                    .await,
                )
            } else if matches!(
                error.kind(),
                PublishErrorKind::Unroutable
                    | PublishErrorKind::Rejected
                    | PublishErrorKind::RejectedBeforeSend
            ) {
                let acquisition = claim
                    .retry
                    .consumed(claim.attempt_count)
                    .expect("validated history");
                match select_delay(acquisition) {
                    Ok(delay) => FailureHandling::Scheduled(
                        observe(
                            observer,
                            Op::Schedule,
                            repository.schedule_retry(ScheduleRetry {
                                owner,
                                delay,
                                evidence,
                                diagnostic: if error.kind() == PublishErrorKind::Unroutable {
                                    RetryDiagnostic::Unroutable
                                } else {
                                    RetryDiagnostic::Rejected
                                },
                            }),
                            observation::retry,
                        )
                        .await,
                    ),
                    Err(error) => FailureHandling::PolicyFailed(error),
                }
            } else if error.kind() == PublishErrorKind::Unavailable {
                // ADR 008 infrastructure recovery: known before-send, not an
                // eligible event failure. Reconnection gates subsequent admission.
                FailureHandling::Released(
                    observe(
                        observer,
                        Op::Release,
                        repository.release(claim.key),
                        observation::transition,
                    )
                    .await,
                )
            } else {
                // Current adapter cannot distinguish a global encoder defect from
                // event-specific correctable serialization. Do not invent evidence.
                FailureHandling::LeaseRetained
            };
            DeliveryOutcome::PublicationFailed {
                claim,
                error,
                handling,
            }
        }
    }
}
