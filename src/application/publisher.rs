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
    Unavailable,
    Serialization,
    Rejected,
    Unroutable,
    Uncertain,
}

/// Public formatting is sanitized. Diagnostic sources must be redacted before logging.
pub struct PublishError {
    kind: PublishErrorKind,
    source: Option<Box<dyn Error + Send + Sync>>,
}
impl PublishError {
    pub fn new(kind: PublishErrorKind) -> Self {
        Self { kind, source: None }
    }
    pub fn with_source(kind: PublishErrorKind, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
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
