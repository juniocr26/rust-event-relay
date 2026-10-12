use chrono::{DateTime, Utc};
use reliable_event_relay::domain::event::{EventEnvelope, EventError, EventType};
use serde_json::{Value, json};
use uuid::Uuid;

fn fixture() -> Value {
    json!({
        "id": "019a5a22-0000-7000-8000-000000000001",
        "event_type": "order.created",
        "aggregate_type": "order",
        "aggregate_id": "12345",
        "schema_version": 1,
        "occurred_at": "2026-10-07T16:00:00Z",
        "correlation_id": null,
        "causation_id": null,
        "payload": {"order_id": "12345"}
    })
}

#[test]
fn creates_event_with_generated_v7_identity_and_utc_time() {
    let before = Utc::now();
    let event = EventEnvelope::new(
        "order.created",
        "order",
        "external:123",
        1,
        json!({"ok": true}),
    )
    .unwrap();
    let after = Utc::now();
    assert_eq!(event.id().get_version_num(), 7);
    assert!(!event.id().is_nil());
    assert!((before..=after).contains(&event.occurred_at()));
    assert_eq!(event.occurred_at().offset(), &Utc);
    assert_eq!(event.event_type().as_str(), "order.created");
    assert_eq!(event.aggregate_type(), "order");
    assert_eq!(event.aggregate_id(), "external:123");
    assert_eq!(event.schema_version(), 1);
    assert_eq!(event.payload(), &json!({"ok": true}));
    assert_eq!(event.correlation_id(), None);
    assert_eq!(event.causation_id(), None);
    let next = EventEnvelope::new("custom.event", "custom", "42", 2, Value::Null).unwrap();
    assert_ne!(event.id(), next.id());
}

#[test]
fn rejects_empty_and_whitespace_identifiers_and_zero_version() {
    for empty in ["", " \t\n"] {
        assert_eq!(
            EventEnvelope::new(empty, "order", "123", 1, Value::Null),
            Err(EventError::EmptyEventType)
        );
        assert_eq!(
            EventEnvelope::new("order.created", empty, "123", 1, Value::Null),
            Err(EventError::EmptyAggregateType)
        );
        assert_eq!(
            EventEnvelope::new("order.created", "order", empty, 1, Value::Null),
            Err(EventError::EmptyAggregateId)
        );
        assert_eq!(EventType::new(empty), Err(EventError::EmptyEventType));
        assert!(serde_json::from_value::<EventType>(json!(empty)).is_err());
    }
    assert_eq!(
        EventEnvelope::new("order.created", "order", "123", 0, Value::Null),
        Err(EventError::ZeroSchemaVersion)
    );
}

#[test]
fn restores_identity_and_time_and_serializes_exact_wire_shape() {
    let expected = fixture();
    let event: EventEnvelope = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(
        event.id(),
        expected["id"].as_str().unwrap().parse::<Uuid>().unwrap()
    );
    assert_eq!(
        event.occurred_at(),
        "2026-10-07T16:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(serde_json::to_value(&event).unwrap(), expected);
}

#[test]
fn round_trip_preserves_generated_event_and_optional_metadata() {
    let correlation = Uuid::now_v7();
    let causation = Uuid::now_v7();
    for event in [
        EventEnvelope::new(
            "inventory.reserved",
            "inventory",
            "SKU-1",
            3,
            json!([1, "x"]),
        )
        .unwrap(),
        EventEnvelope::new("payment.completed", "payment", "42", 1, Value::Null)
            .unwrap()
            .with_correlation_id(correlation),
        EventEnvelope::new("custom.event", "custom", "abc", 1, json!(true))
            .unwrap()
            .with_causation_id(causation),
        EventEnvelope::new("custom.event", "custom", "abc", 1, json!({}))
            .unwrap()
            .with_correlation_id(correlation)
            .with_causation_id(causation),
    ] {
        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(encoded["correlation_id"], json!(event.correlation_id()));
        assert_eq!(encoded["causation_id"], json!(event.causation_id()));
        let restored: EventEnvelope =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(restored, event);
    }
}

#[test]
fn deserialization_enforces_envelope_validation() {
    for field in ["event_type", "aggregate_type", "aggregate_id"] {
        for invalid in [json!(""), json!(" \t"), Value::Null] {
            let mut value = fixture();
            value[field] = invalid;
            assert!(
                serde_json::from_value::<EventEnvelope>(value).is_err(),
                "{field}"
            );
        }
    }
    for invalid in [json!(0), json!(-1), json!(1.5), json!(4294967296_u64)] {
        let mut value = fixture();
        value["schema_version"] = invalid;
        assert!(serde_json::from_value::<EventEnvelope>(value).is_err());
    }
    for field in ["id", "occurred_at", "correlation_id", "causation_id"] {
        let mut value = fixture();
        value[field] = json!("invalid");
        assert!(serde_json::from_value::<EventEnvelope>(value).is_err());
    }
}

#[test]
fn normalizes_offset_timestamps_to_utc() {
    let mut value = fixture();
    value["occurred_at"] = json!("2026-10-07T13:00:00-03:00");
    let event: EventEnvelope = serde_json::from_value(value).unwrap();
    assert_eq!(
        serde_json::to_value(event).unwrap()["occurred_at"],
        fixture()["occurred_at"]
    );
}
