//! Validated inventory demonstration; no database or broker effects.
use crate::domain::event::EventEnvelope;
use uuid::Uuid;

/// Invalid input categories carry no raw payload or diagnostic secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidEvent {
    Identity,
    Unsupported,
    Payload,
    Identifier,
    Quantity,
    Overflow,
}

impl std::fmt::Display for InvalidEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid inventory event: {self:?}")
    }
}
impl std::error::Error for InvalidEvent {}

/// Stable logical responsibility; replicas must use the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerName(String);
impl ConsumerName {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidEvent> {
        let value = value.into();
        validate_identifier(&value)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn validate_identifier(value: &str) -> Result<(), InvalidEvent> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(InvalidEvent::Identifier);
    }
    Ok(())
}

/// Nonnegative PostgreSQL BIGINT-compatible stock; absent rows start at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stock(i64);
impl Stock {
    pub fn new(value: i64) -> Result<Self, InvalidEvent> {
        if value < 0 {
            return Err(InvalidEvent::Quantity);
        }
        Ok(Self(value))
    }
    pub fn units(self) -> i64 {
        self.0
    }
    pub fn receive(self, event: &InventoryEvent) -> Result<Self, InvalidEvent> {
        self.0
            .checked_add(event.quantity)
            .map(Self)
            .ok_or(InvalidEvent::Overflow)
    }
}

/// Checked representation v1. Envelope equality includes all immutable fields.
/// JSON object order is ignored; arbitrary-precision number spelling is retained.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryEvent {
    envelope: EventEnvelope,
    warehouse_id: String,
    sku: String,
    quantity: i64,
}
impl InventoryEvent {
    pub fn validate(
        envelope: EventEnvelope,
        message_id: Option<&str>,
    ) -> Result<Self, InvalidEvent> {
        if envelope.id().is_nil()
            || message_id.and_then(|s| Uuid::parse_str(s).ok()) != Some(envelope.id())
        {
            return Err(InvalidEvent::Identity);
        }
        if envelope.event_type().as_str() != "inventory.units_received"
            || envelope.schema_version() != 1
        {
            return Err(InvalidEvent::Unsupported);
        }
        validate_identifier(envelope.aggregate_type())?;
        validate_identifier(envelope.aggregate_id())?;
        let object = envelope
            .payload()
            .as_object()
            .ok_or(InvalidEvent::Payload)?;
        if object.len() != 3 {
            return Err(InvalidEvent::Payload);
        }
        let warehouse_id = object
            .get("warehouse_id")
            .and_then(|v| v.as_str())
            .ok_or(InvalidEvent::Payload)?
            .to_owned();
        let sku = object
            .get("sku")
            .and_then(|v| v.as_str())
            .ok_or(InvalidEvent::Payload)?
            .to_owned();
        validate_identifier(&warehouse_id)?;
        validate_identifier(&sku)?;
        let quantity = object
            .get("quantity")
            .and_then(|v| v.as_i64())
            .filter(|q| *q > 0)
            .ok_or(InvalidEvent::Quantity)?;
        Ok(Self {
            envelope,
            warehouse_id,
            sku,
            quantity,
        })
    }
    pub fn event_id(&self) -> Uuid {
        self.envelope.id()
    }
    pub fn envelope(&self) -> &EventEnvelope {
        &self.envelope
    }
    pub fn warehouse_id(&self) -> &str {
        &self.warehouse_id
    }
    pub fn sku(&self) -> &str {
        &self.sku
    }
    pub fn quantity(&self) -> i64 {
        self.quantity
    }
    pub fn compare(&self, prior: &Self) -> ContentMatch {
        if self.event_id() != prior.event_id() {
            ContentMatch::Distinct
        } else if self == prior {
            ContentMatch::Duplicate
        } else {
            ContentMatch::IdentityConflict
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentMatch {
    Distinct,
    Duplicate,
    IdentityConflict,
}
