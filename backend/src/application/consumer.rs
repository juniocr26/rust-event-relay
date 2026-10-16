//! Future atomic processing and quarantine boundaries, with manual ACK policy.
use super::publisher::PublishError;
use crate::domain::consumer::{ConsumerName, InvalidEvent, InventoryEvent, Stock};
use std::future::Future;

/// Historical effect receipt; duplicate stock is the original post-effect value,
/// not a current stock observation. Returned only after acknowledged commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectReceipt {
    pub stock_after: Stock,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingOutcome {
    Applied(EffectReceipt),
    Duplicate(EffectReceipt),
    IdentityConflict,
    Invalid(InvalidEvent),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingFailure {
    PreCommit,
    /// Persisted content or receipt cannot be faithfully restored.
    InvalidStoredData,
    CommitUncertain,
}
/// One transaction owns (consumer,event), increments stock and stores the receipt.
/// Cancellation never proves rollback. No external side effects are covered.
pub trait AtomicInventoryProcessor {
    fn process(
        &self,
        consumer: &ConsumerName,
        event: &InventoryEvent,
    ) -> impl Future<Output = Result<ProcessingOutcome, ProcessingFailure>> + Send;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryAction {
    Ack,
    Preserve,
    RecoverWithPacing,
}
pub fn delivery_action(result: Result<ProcessingOutcome, ProcessingFailure>) -> DeliveryAction {
    match result {
        Ok(ProcessingOutcome::Applied(_) | ProcessingOutcome::Duplicate(_)) => DeliveryAction::Ack,
        Ok(ProcessingOutcome::IdentityConflict | ProcessingOutcome::Invalid(_)) => {
            DeliveryAction::Preserve
        }
        Err(_) => DeliveryAction::RecoverWithPacing,
    }
}
/// Closed diagnostic vocabulary: bounded, no raw errors or user content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReason {
    InvalidIdentity,
    InvalidContent,
    Unsupported,
    IdentityConflict,
    StockOverflow,
}
/// Preserves original bytes and message_id (including missing/malformed identity).
/// No truncation: an oversized body must remain unacknowledged for operator recovery.
/// Adapters preserve original AMQP properties in the type-tagged JSON sidecar;
/// no credentials or free-form exception diagnostics may be added.
pub struct QuarantineMessage {
    body: Vec<u8>,
    properties: serde_json::Value,
    message_id: Option<String>,
    reason: QuarantineReason,
}
impl QuarantineMessage {
    pub const MAX_BODY_BYTES: usize = 1024 * 1024;
    pub const MAX_MESSAGE_ID_BYTES: usize = 255;
    pub const MAX_PROPERTIES_BYTES: usize = 16 * 1024;
    pub fn new(
        body: Vec<u8>,
        properties: serde_json::Value,
        message_id: Option<String>,
        reason: QuarantineReason,
    ) -> Option<Self> {
        if !properties.is_object()
            || serde_json::to_vec(&properties).ok()?.len() > Self::MAX_PROPERTIES_BYTES
            || body.len() > Self::MAX_BODY_BYTES
            || message_id
                .as_ref()
                .is_some_and(|id| id.len() > Self::MAX_MESSAGE_ID_BYTES)
        {
            return None;
        }
        Some(Self {
            body,
            properties,
            message_id,
            reason,
        })
    }
    pub fn properties(&self) -> &serde_json::Value {
        &self.properties
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn message_id(&self) -> Option<&str> {
        self.message_id.as_deref()
    }
    pub fn reason(&self) -> QuarantineReason {
        self.reason
    }
}
/// Ok proves persistent mandatory publication to the configured durable quarantine
/// target was confirmed without return. Nack/return/timeout/cancellation cannot ACK.
/// Repeated publication is allowed: there is no atomic publish-and-source-ACK.
pub trait QuarantinePublisher {
    fn preserve(
        &self,
        message: &QuarantineMessage,
    ) -> impl Future<Output = Result<(), PublishError>> + Send;
}
/// Apply only to the matching publication attempt and original channel/delivery.
pub fn preservation_action(result: &Result<(), PublishError>) -> DeliveryAction {
    if result.is_ok() {
        DeliveryAction::Ack
    } else {
        DeliveryAction::RecoverWithPacing
    }
}
