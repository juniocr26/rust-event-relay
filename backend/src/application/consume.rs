//! One delivery: durable processing or confirmed preservation precedes individual ACK.
//! Uma entrega: processamento durável ou preservação confirmada precede ACK individual.
use super::observation::{self, Noop, Observer, Operation as Op, Outcome as Obs, observe};
use super::{
    consumer::*,
    publisher::{PublishError, PublishErrorKind},
};
use crate::domain::{
    consumer::{ConsumerName, InvalidEvent, InventoryEvent},
    event::EventEnvelope,
};
use std::{future::Future, time::Duration};
use tokio::time::timeout;

/// Transport-owned bytes/identity and original-channel acknowledgement.
/// Bytes/identidade do transporte e confirmação no canal original.
pub trait IncomingDelivery: Sync {
    fn body(&self) -> &[u8];
    fn message_id(&self) -> Option<&str>;
    /// None means exact preservation exceeds supported bounds: retain source.
    /// None significa preservação fora dos limites: mantenha a origem.
    fn preservation(
        &self,
        consumer: &ConsumerName,
        reason: QuarantineReason,
    ) -> Option<QuarantineMessage>;
    fn ack(&self) -> impl Future<Output = Result<(), PublishError>> + Send;
}
#[derive(Debug)]
pub enum PrimaryOutcome {
    Processing(Result<ProcessingOutcome, ProcessingFailure>),
    Invalid(QuarantineReason),
}
/// Primary business evidence survives secondary preservation/ACK errors.
/// Evidência primária é preservada mesmo com erro de preservação/ACK.
#[derive(Debug)]
pub struct HandlingOutcome {
    pub primary: PrimaryOutcome,
    pub preservation: Option<Result<(), PublishError>>,
    pub acknowledgement: Option<Result<(), PublishError>>,
    pub preservation_blocked: bool,
}
impl HandlingOutcome {
    pub fn recover(&self) -> bool {
        !matches!(self.acknowledgement, Some(Ok(())))
    }
}
fn reason(error: InvalidEvent) -> QuarantineReason {
    match error {
        InvalidEvent::Identity => QuarantineReason::InvalidIdentity,
        InvalidEvent::Unsupported => QuarantineReason::Unsupported,
        InvalidEvent::Overflow => QuarantineReason::StockOverflow,
        _ => QuarantineReason::InvalidContent,
    }
}
/// No retries or compensating ACK on cancellation. Each phase has a finite deadline.
/// Sem retries ou ACK compensatório no cancelamento. Cada fase tem prazo finito.
pub async fn handle_delivery<
    D: IncomingDelivery,
    P: AtomicInventoryProcessor,
    Q: QuarantinePublisher,
>(
    delivery: &D,
    processor: &P,
    quarantine: &Q,
    consumer: &ConsumerName,
    deadline: Duration,
) -> HandlingOutcome {
    handle_delivery_observed(delivery, processor, quarantine, consumer, deadline, &Noop).await
}
pub async fn handle_delivery_observed<
    D: IncomingDelivery,
    P: AtomicInventoryProcessor,
    Q: QuarantinePublisher,
    O: Observer,
>(
    delivery: &D,
    processor: &P,
    quarantine: &Q,
    consumer: &ConsumerName,
    deadline: Duration,
    observer: &O,
) -> HandlingOutcome {
    // Bound parse cost before serde (which also applies its recursion limit).
    let parsed = if delivery.body().len() > QuarantineMessage::MAX_BODY_BYTES {
        Err(QuarantineReason::Unsupported)
    } else {
        serde_json::from_slice::<serde_json::Value>(delivery.body())
            .map_err(|_| QuarantineReason::MalformedJson)
            .and_then(|value| {
                serde_json::from_value::<EventEnvelope>(value)
                    .map_err(|_| QuarantineReason::InvalidContent)
            })
            .and_then(|envelope| {
                InventoryEvent::validate(envelope, delivery.message_id()).map_err(reason)
            })
    };
    let primary = match parsed {
        Ok(event) => PrimaryOutcome::Processing(
            observe(
                observer,
                Op::Processing,
                async {
                    timeout(deadline, processor.process(consumer, &event))
                        .await
                        // Whole processing timeout can race COMMIT, never proves rollback.
                        .unwrap_or_else(|_| {
                            Err(ProcessingFailure::new(
                                ProcessingFailureKind::CommitUncertain,
                            ))
                        })
                },
                |r| match r {
                    Ok(ProcessingOutcome::Applied(_)) => Obs::Applied,
                    Ok(ProcessingOutcome::Duplicate(_)) => Obs::Duplicate,
                    Ok(ProcessingOutcome::IdentityConflict) => Obs::IdentityConflict,
                    Ok(ProcessingOutcome::Invalid(_)) => Obs::Invalid,
                    Err(e) => match e.kind() {
                        ProcessingFailureKind::CommitUncertain => Obs::CommitUncertain,
                        ProcessingFailureKind::Unavailable => Obs::Unavailable,
                        ProcessingFailureKind::InvalidStoredData => Obs::Invalid,
                        ProcessingFailureKind::PreCommit => Obs::Failed,
                    },
                },
            )
            .await,
        ),
        Err(reason) => {
            observer.event(Op::Processing, Obs::Invalid);
            PrimaryOutcome::Invalid(reason)
        }
    };
    let preserve = match &primary {
        PrimaryOutcome::Processing(Ok(
            ProcessingOutcome::Applied(_) | ProcessingOutcome::Duplicate(_),
        )) => None,
        PrimaryOutcome::Processing(Ok(ProcessingOutcome::IdentityConflict)) => {
            Some(QuarantineReason::IdentityConflict)
        }
        PrimaryOutcome::Processing(Ok(ProcessingOutcome::Invalid(error))) => Some(reason(*error)),
        PrimaryOutcome::Processing(Err(_)) => {
            return HandlingOutcome {
                primary,
                preservation: None,
                acknowledgement: None,
                preservation_blocked: false,
            };
        }
        PrimaryOutcome::Invalid(reason) => Some(*reason),
    };
    let mut outcome = HandlingOutcome {
        primary,
        preservation: None,
        acknowledgement: None,
        preservation_blocked: false,
    };
    if let Some(reason) = preserve {
        let Some(message) = delivery.preservation(consumer, reason) else {
            observer.event(Op::Quarantine, Obs::Blocked);
            outcome.preservation_blocked = true;
            return outcome;
        };
        let result = observe(
            observer,
            Op::Quarantine,
            async {
                timeout(deadline, quarantine.preserve(&message))
                    .await
                    .unwrap_or_else(|_| Err(PublishError::new(PublishErrorKind::Uncertain)))
            },
            observation::publication,
        )
        .await;
        let confirmed = result.is_ok();
        outcome.preservation = Some(result);
        if !confirmed {
            return outcome;
        }
    }
    outcome.acknowledgement = Some(
        observe(
            observer,
            Op::Ack,
            async {
                timeout(deadline, delivery.ack())
                    .await
                    .unwrap_or_else(|_| Err(PublishError::new(PublishErrorKind::Uncertain)))
            },
            |r| {
                if r.is_ok() {
                    Obs::Completed
                } else {
                    observation::publication(r)
                }
            },
        )
        .await,
    );
    outcome
}
