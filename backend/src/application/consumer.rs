//! Atomic processing and future quarantine boundaries, with manual ACK policy.
//! Processamento atômico e fronteiras futuras de quarentena, com política de ACK manual.
use super::publisher::PublishError;
use crate::domain::consumer::{ConsumerName, InvalidEvent, InventoryEvent, Stock};
use crate::persistence::{PersistenceError, PersistenceErrorKind};
use std::{error::Error, fmt, future::Future};

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
/// Sanitized processing classification; no automatic retry policy.
/// Classificação sanitizada; não define repetição automática.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingFailureKind {
    /// Failure before COMMIT, or known COMMIT rejection; no committed effect.
    /// Falha anterior ao COMMIT ou rejeição conhecida dele; sem efeito commitado.
    PreCommit,
    /// Connection, pool or infrastructure unavailable before COMMIT.
    /// Conexão, pool ou infraestrutura indisponível antes do COMMIT.
    Unavailable,
    /// Persisted content or receipt cannot be faithfully restored.
    /// Conteúdo ou recibo persistido não pode ser restaurado fielmente.
    InvalidStoredData,
    /// COMMIT may have succeeded; preserve identity during recovery.
    /// COMMIT pode ter ocorrido; preserve a identidade na recuperação.
    CommitUncertain,
}
/// Reuses persistence diagnostics without exposing driver or content in formatting.
/// Reutiliza diagnósticos sem expor driver ou conteúdo na formatação.
#[derive(Debug)]
pub struct ProcessingFailure(PersistenceError);
impl ProcessingFailure {
    /// Creates a source-free failure, useful at application boundaries.
    /// Cria uma falha sem causa para fronteiras da aplicação.
    pub fn new(kind: ProcessingFailureKind) -> Self {
        Self(PersistenceError::new(match kind {
            ProcessingFailureKind::PreCommit => PersistenceErrorKind::OperationFailed,
            ProcessingFailureKind::Unavailable => PersistenceErrorKind::Unavailable,
            ProcessingFailureKind::InvalidStoredData => PersistenceErrorKind::InvalidStoredData,
            ProcessingFailureKind::CommitUncertain => PersistenceErrorKind::CommitUncertain,
        }))
    }
    /// Stable classification independent of SQLx.
    /// Classificação estável independente de SQLx.
    pub fn kind(&self) -> ProcessingFailureKind {
        match self.0.kind() {
            PersistenceErrorKind::OperationFailed => ProcessingFailureKind::PreCommit,
            PersistenceErrorKind::Unavailable => ProcessingFailureKind::Unavailable,
            PersistenceErrorKind::InvalidStoredData => ProcessingFailureKind::InvalidStoredData,
            PersistenceErrorKind::CommitUncertain => ProcessingFailureKind::CommitUncertain,
        }
    }
}
impl From<PersistenceError> for ProcessingFailure {
    fn from(error: PersistenceError) -> Self {
        Self(error)
    }
}
impl fmt::Display for ProcessingFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind() {
            ProcessingFailureKind::PreCommit => "consumer transaction rejected",
            ProcessingFailureKind::Unavailable => "consumer persistence unavailable",
            ProcessingFailureKind::InvalidStoredData => "invalid stored consumer data",
            ProcessingFailureKind::CommitUncertain => "consumer commit uncertain",
        })
    }
}
impl Error for ProcessingFailure {
    // Typed sources are for controlled diagnostics; redact before logging.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QuarantineReason {
    MalformedJson,
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
            || !properties_fit(&properties)
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

/// Preflight caller-owned trees before allocating encoded diagnostics.
/// Bounds nodes, depth and raw strings, then caps escaped JSON bytes while writing.
pub fn properties_fit(value: &serde_json::Value) -> bool {
    fn visit(
        value: &serde_json::Value,
        depth: usize,
        nodes: &mut usize,
        bytes: &mut usize,
    ) -> Option<()> {
        *nodes = nodes.checked_sub(1)?;
        if depth > 64 {
            return None;
        }
        match value {
            serde_json::Value::String(s) => *bytes = bytes.checked_sub(s.len())?,
            serde_json::Value::Array(a) => {
                for item in a {
                    visit(item, depth + 1, nodes, bytes)?;
                }
            }
            serde_json::Value::Object(o) => {
                for (key, item) in o {
                    *bytes = bytes.checked_sub(key.len())?;
                    visit(item, depth + 1, nodes, bytes)?;
                }
            }
            _ => {}
        }
        Some(())
    }
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("preservation byte budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut nodes = 4096;
    let mut bytes = QuarantineMessage::MAX_PROPERTIES_BYTES;
    visit(value, 0, &mut nodes, &mut bytes).is_some()
        && serde_json::to_writer(Counter(QuarantineMessage::MAX_PROPERTIES_BYTES), value).is_ok()
}
