use std::{error::Error, fmt};

/// Caller-visible classification; no driver codes or automatic retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceErrorKind {
    /// Storage could not be reached or used; this does not promise retry success.
    Unavailable,
    /// Selected stored values violate the model or cannot be decoded faithfully.
    InvalidStoredData,
    /// Another storage operation failure; classification remains adapter-owned.
    OperationFailed,
    /// A mutation may have committed; never interpret this as a known rollback.
    CommitUncertain,
}

/// Storage error with optional diagnostic source and sanitized public formatting.
///
/// Display and Debug omit source text, connection strings and event contents.
/// Sources may contain sensitive details: callers must redact before logging them.
pub struct PersistenceError {
    kind: PersistenceErrorKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl PersistenceError {
    /// Creates a classified error without an underlying diagnostic source.
    pub fn new(kind: PersistenceErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Preserves a source for controlled diagnostics without formatting it publicly.
    pub fn with_source(
        kind: PersistenceErrorKind,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    /// Classification application code can match without importing a driver.
    pub fn kind(&self) -> PersistenceErrorKind {
        self.kind
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            PersistenceErrorKind::Unavailable => "persistence unavailable",
            PersistenceErrorKind::InvalidStoredData => "invalid stored outbox data",
            PersistenceErrorKind::OperationFailed => "persistence operation failed",
            PersistenceErrorKind::CommitUncertain => "persistence commit uncertain",
        })
    }
}

impl fmt::Debug for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistenceError")
            .field("kind", &self.kind)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl Error for PersistenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as &dyn Error)
    }
}
