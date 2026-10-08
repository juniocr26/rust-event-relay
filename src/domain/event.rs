//! Canonical transport/storage envelope. Business logic may use typed payloads.
use std::{error::Error, fmt, num::NonZeroU32};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Envelope validation failures, shared by construction and JSON restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    EmptyEventType,
    EmptyAggregateType,
    EmptyAggregateId,
    ZeroSchemaVersion,
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyEventType => "event_type must not be empty",
            Self::EmptyAggregateType => "aggregate_type must not be empty",
            Self::EmptyAggregateId => "aggregate_id must not be empty",
            Self::ZeroSchemaVersion => "schema_version must be greater than zero",
        })
    }
}
impl Error for EventError {}

/// Application-defined logical name; no business event catalog is imposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventType(String);

impl EventType {
    pub fn new(value: impl Into<String>) -> Result<Self, EventError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EventError::EmptyEventType);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventType {
    type Error = EventError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<EventType> for String {
    fn from(value: EventType) -> Self {
        value.0
    }
}

/// Stable event identity and metadata, independent of persistence or delivery.
/// Private fields preserve validation; deserialization restores the original ID/time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EventEnvelopeParts")]
pub struct EventEnvelope {
    id: Uuid,
    event_type: EventType,
    aggregate_type: String,
    aggregate_id: String,
    schema_version: NonZeroU32,
    occurred_at: DateTime<Utc>,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    payload: Value,
}

impl EventEnvelope {
    /// Generates a UUID v7 and the current UTC occurrence time.
    /// UUID time ordering does not imply strict global event ordering.
    pub fn new(
        event_type: impl Into<String>,
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        schema_version: u32,
        payload: Value,
    ) -> Result<Self, EventError> {
        let event_type = EventType::new(event_type)?;
        let aggregate_type = aggregate_type.into();
        let aggregate_id = aggregate_id.into();
        let schema_version = validate(&aggregate_type, &aggregate_id, schema_version)?;
        Ok(Self {
            id: Uuid::now_v7(),
            event_type,
            aggregate_type,
            aggregate_id,
            schema_version,
            occurred_at: Utc::now(),
            correlation_id: None,
            causation_id: None,
            payload,
        })
    }

    /// Restores existing values using the same validation as deserialization.
    pub fn restore(parts: EventEnvelopeParts) -> Result<Self, EventError> {
        parts.try_into()
    }

    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    pub fn with_causation_id(mut self, id: Uuid) -> Self {
        self.causation_id = Some(id);
        self
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }
    pub fn aggregate_type(&self) -> &str {
        &self.aggregate_type
    }
    pub fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }
    pub fn schema_version(&self) -> u32 {
        self.schema_version.get()
    }
    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }
    pub fn correlation_id(&self) -> Option<Uuid> {
        self.correlation_id
    }
    pub fn causation_id(&self) -> Option<Uuid> {
        self.causation_id
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

fn validate(
    aggregate_type: &str,
    aggregate_id: &str,
    version: u32,
) -> Result<NonZeroU32, EventError> {
    if aggregate_type.trim().is_empty() {
        return Err(EventError::EmptyAggregateType);
    }
    if aggregate_id.trim().is_empty() {
        return Err(EventError::EmptyAggregateId);
    }
    NonZeroU32::new(version).ok_or(EventError::ZeroSchemaVersion)
}

/// Restoration input, validated when converted into an envelope.
/// Identity and timestamps are supplied by the caller and never regenerated.
#[derive(Deserialize)]
pub struct EventEnvelopeParts {
    pub id: Uuid,
    pub event_type: EventType,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub schema_version: u32,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub payload: Value,
}

impl TryFrom<EventEnvelopeParts> for EventEnvelope {
    type Error = EventError;
    fn try_from(value: EventEnvelopeParts) -> Result<Self, Self::Error> {
        let schema_version = validate(
            &value.aggregate_type,
            &value.aggregate_id,
            value.schema_version,
        )?;
        Ok(Self {
            id: value.id,
            event_type: value.event_type,
            aggregate_type: value.aggregate_type,
            aggregate_id: value.aggregate_id,
            schema_version,
            occurred_at: value.occurred_at,
            correlation_id: value.correlation_id,
            causation_id: value.causation_id,
            payload: value.payload,
        })
    }
}
