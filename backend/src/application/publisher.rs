//! Publication boundary: confirmed broker acceptance is not consumer processing.
use crate::domain::event::EventEnvelope;
use std::{error::Error, fmt, future::Future};

/// Static-dispatch contract; adapters keep protocol types outside this boundary.
/// Dropping a publication future after sending may leave acceptance uncertain.
pub trait EventPublisher {
    fn publish(
        &self,
        event: &EventEnvelope,
    ) -> impl Future<Output = Result<(), PublishError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishErrorKind {
    /// Before-send admission/resource failure; no acceptance in this invocation.
    Unavailable,
    /// Before-send encoding failure; category alone proves no permanence.
    Serialization,
    /// Definitive negative confirmation; no required acceptance established.
    Rejected,
    /// Explicit bounded rejection before transport send; eligible for durable retry.
    RejectedBeforeSend,
    /// Definitive failure of required routing; topology may be repairable.
    Unroutable,
    /// Sending may have begun; acceptance is unknown. Never compensate blindly.
    Uncertain,
}

/// Public formatting is sanitized. Diagnostic sources must be redacted before logging.
pub struct PublishError {
    kind: PublishErrorKind,
    source: Option<Box<dyn Error + Send + Sync>>,
    permanent_evidence: Option<crate::domain::retry::FailureEvidence>,
}
impl PublishError {
    pub fn new(kind: PublishErrorKind) -> Self {
        Self {
            kind,
            source: None,
            permanent_evidence: None,
        }
    }
    pub fn with_source(kind: PublishErrorKind, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
            permanent_evidence: None,
        }
    }
    /// Explicit adapter/caller attestation of a reproducible event-specific encoder
    /// defect. Generic serialization errors must use the ordinary constructors.
    /// This grants terminal policy authority; it is not inferred from the category.
    pub fn permanent_serialization(evidence: crate::domain::retry::FailureEvidence) -> Self {
        Self {
            kind: PublishErrorKind::Serialization,
            source: None,
            permanent_evidence: Some(evidence),
        }
    }
    pub fn permanent_evidence(&self) -> Option<crate::domain::retry::FailureEvidence> {
        self.permanent_evidence
    }
    pub fn kind(&self) -> PublishErrorKind {
        self.kind
    }
}
impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "publication failed: {:?}", self.kind)
    }
}
impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl Error for PublishError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}
