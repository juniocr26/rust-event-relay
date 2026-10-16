use reliable_event_relay::{
    application::{
        consumer::*,
        publisher::{PublishError, PublishErrorKind},
    },
    domain::{consumer::*, event::EventEnvelope},
};
use serde_json::{Value, json};
fn envelope(quantity: Value) -> EventEnvelope {
    EventEnvelope::new(
        "inventory.units_received",
        "stock",
        "item-1",
        1,
        json!({"warehouse_id":"local", "sku":"SKU-1", "quantity":quantity}),
    )
    .unwrap()
}
fn checked(event: EventEnvelope) -> InventoryEvent {
    let id = event.id().to_string();
    InventoryEvent::validate(event, Some(&id)).unwrap()
}
#[test]
fn quantity_and_stock_boundaries() {
    for q in [
        json!(0),
        json!(-1),
        json!(1.0),
        json!("5"),
        json!(9223372036854775808_u64),
    ] {
        let event = envelope(q);
        assert_eq!(
            InventoryEvent::validate(event.clone(), Some(&event.id().to_string())),
            Err(InvalidEvent::Quantity)
        );
    }
    let five = checked(envelope(json!(5)));
    assert_eq!(Stock::new(10).unwrap().receive(&five).unwrap().units(), 15);
    assert_eq!(
        Stock::new(i64::MAX - 5)
            .unwrap()
            .receive(&five)
            .unwrap()
            .units(),
        i64::MAX
    );
    assert_eq!(
        Stock::new(i64::MAX - 4).unwrap().receive(&five),
        Err(InvalidEvent::Overflow)
    );
    assert!(Stock::new(-1).is_err());
    assert_eq!(checked(envelope(json!(i64::MAX))).quantity(), i64::MAX);
}
#[test]
fn stable_identity_and_identifier_bounds() {
    assert_eq!(
        ConsumerName::new("inventory.stock.v1"),
        ConsumerName::new("inventory.stock.v1")
    );
    assert_ne!(
        ConsumerName::new("inventory.stock.v1"),
        ConsumerName::new("audit.v1")
    );
    assert!(ConsumerName::new("a".repeat(128)).is_ok());
    for name in [
        "".to_string(),
        "a".repeat(129),
        "white space".into(),
        "á".into(),
    ] {
        assert!(ConsumerName::new(name).is_err());
    }
    let event = envelope(json!(5));
    for id in [
        None,
        Some("malformed"),
        Some("00000000-0000-0000-0000-000000000000"),
    ] {
        assert_eq!(
            InventoryEvent::validate(event.clone(), id),
            Err(InvalidEvent::Identity)
        );
    }
    for field in ["warehouse_id", "sku", "aggregate_id", "aggregate_type"] {
        let mut value = serde_json::to_value(&event).unwrap();
        if field.starts_with("aggregate") {
            value[field] = json!("a".repeat(129));
        } else {
            value["payload"][field] = json!("a".repeat(129));
        }
        let restored: EventEnvelope = serde_json::from_value(value).unwrap();
        assert_eq!(
            InventoryEvent::validate(restored, Some(&event.id().to_string())),
            Err(InvalidEvent::Identifier)
        );
    }
}
#[test]
fn supported_schema_and_immutable_comparison() {
    let original = envelope(json!(5));
    let event = checked(original.clone());
    let reordered = serde_json::to_string(&original).unwrap().replace(
        "\"quantity\":5,\"sku\":\"SKU-1\",\"warehouse_id\":\"local\"",
        "\"warehouse_id\":\"local\",\"quantity\":5,\"sku\":\"SKU-1\"",
    );
    assert_eq!(
        event.compare(&checked(serde_json::from_str(&reordered).unwrap())),
        ContentMatch::Duplicate
    );
    assert_eq!(
        event.compare(&checked(envelope(json!(5)))),
        ContentMatch::Distinct
    );
    for field in [
        "aggregate_id",
        "correlation_id",
        "causation_id",
        "occurred_at",
        "quantity",
        "warehouse_id",
        "sku",
    ] {
        let mut value = serde_json::to_value(&original).unwrap();
        match field {
            "quantity" => value["payload"][field] = json!(6),
            "warehouse_id" | "sku" => value["payload"][field] = json!("other"),
            "correlation_id" | "causation_id" => value[field] = json!(uuid::Uuid::now_v7()),
            "occurred_at" => value[field] = json!("2000-01-01T00:00:00Z"),
            _ => value[field] = json!("other"),
        }
        assert_eq!(
            event.compare(&checked(serde_json::from_value(value).unwrap())),
            ContentMatch::IdentityConflict
        );
    }
    for (field, replacement) in [("event_type", json!("other")), ("schema_version", json!(2))] {
        let mut value = serde_json::to_value(&original).unwrap();
        value[field] = replacement;
        assert_eq!(
            InventoryEvent::validate(
                serde_json::from_value(value).unwrap(),
                Some(&original.id().to_string())
            ),
            Err(InvalidEvent::Unsupported)
        );
    }
}
#[test]
fn ack_requires_commit_or_verified_duplicate() {
    let receipt = EffectReceipt {
        stock_after: Stock::new(15).unwrap(),
    };
    for outcome in [
        ProcessingOutcome::Applied(receipt),
        ProcessingOutcome::Duplicate(receipt),
    ] {
        assert_eq!(delivery_action(Ok(outcome)), DeliveryAction::Ack);
    }
    for outcome in [
        ProcessingOutcome::IdentityConflict,
        ProcessingOutcome::Invalid(InvalidEvent::Identity),
    ] {
        assert_eq!(delivery_action(Ok(outcome)), DeliveryAction::Preserve);
    }
    for error in [
        ProcessingFailureKind::PreCommit,
        ProcessingFailureKind::Unavailable,
        ProcessingFailureKind::InvalidStoredData,
        ProcessingFailureKind::CommitUncertain,
    ] {
        assert_eq!(
            delivery_action(Err(ProcessingFailure::new(error))),
            DeliveryAction::RecoverWithPacing
        );
    }
}
#[tokio::test]
async fn crash_or_uncertain_source_ack_can_duplicate_quarantine_publication() {
    use std::sync::Mutex;
    type Preserved = (Vec<u8>, serde_json::Value, Option<String>);
    struct Recorder(Mutex<Vec<Preserved>>);
    impl QuarantinePublisher for Recorder {
        async fn preserve(&self, message: &QuarantineMessage) -> Result<(), PublishError> {
            self.0.lock().unwrap().push((
                message.body().to_vec(),
                message.properties().clone(),
                message.message_id().map(str::to_owned),
            ));
            Ok(())
        }
    }
    let publisher = Recorder(Mutex::new(vec![]));
    let message = QuarantineMessage::new(
        b"invalid JSON\xff".to_vec(),
        json!({"content_type":{"type":"longstr","value":"application/json"}}),
        None,
        QuarantineReason::InvalidIdentity,
    )
    .unwrap();
    // Confirmation happened; process dies before ACK or ACK delivery is unknown.
    assert_eq!(
        preservation_action(&publisher.preserve(&message).await),
        DeliveryAction::Ack
    );
    // Redelivery republishes unchanged bytes; no invented event ID/dedup guarantee.
    assert_eq!(
        preservation_action(&publisher.preserve(&message).await),
        DeliveryAction::Ack
    );
    assert_eq!(
        *publisher.0.lock().unwrap(),
        vec![(message.body().to_vec(), message.properties().clone(), None); 2]
    );
    assert_eq!(message.message_id(), None);
    for kind in [
        PublishErrorKind::Unroutable,
        PublishErrorKind::Rejected,
        PublishErrorKind::Uncertain,
        PublishErrorKind::Unavailable,
    ] {
        assert_eq!(
            preservation_action(&Err(PublishError::new(kind))),
            DeliveryAction::RecoverWithPacing
        );
    }
    assert!(
        QuarantineMessage::new(
            vec![0; QuarantineMessage::MAX_BODY_BYTES + 1],
            json!({}),
            None,
            QuarantineReason::InvalidContent
        )
        .is_none()
    );
    assert!(
        QuarantineMessage::new(
            vec![],
            json!({}),
            Some("x".repeat(256)),
            QuarantineReason::InvalidIdentity
        )
        .is_none()
    );
}

#[test]
fn preservation_bounds_do_not_truncate_evidence() {
    let max = QuarantineMessage::new(
        vec![0; QuarantineMessage::MAX_BODY_BYTES],
        json!({}),
        Some("x".repeat(255)),
        QuarantineReason::InvalidContent,
    )
    .unwrap();
    assert_eq!(max.body().len(), QuarantineMessage::MAX_BODY_BYTES);
    assert_eq!(max.message_id().unwrap().len(), 255);
    assert!(
        QuarantineMessage::new(
            vec![],
            json!({"original": "x".repeat(QuarantineMessage::MAX_PROPERTIES_BYTES)}),
            None,
            QuarantineReason::InvalidContent
        )
        .is_none()
    );
    assert!(
        QuarantineMessage::new(vec![], json!([]), None, QuarantineReason::InvalidContent).is_none()
    );
}

#[test]
fn quarantine_preflight_bounds_raw_escaped_bytes_nodes_and_depth() {
    use reliable_event_relay::application::consumer::{
        QuarantineMessage, QuarantineReason, properties_fit,
    };
    let max = QuarantineMessage::MAX_PROPERTIES_BYTES;
    let exact = serde_json::json!({"x":"x".repeat(max-8)});
    assert_eq!(serde_json::to_vec(&exact).unwrap().len(), max);
    assert!(properties_fit(&exact));
    assert!(!properties_fit(&serde_json::json!({"x":"x".repeat(max-7)})));
    assert!(!properties_fit(
        &serde_json::json!({"x":"\u{0}".repeat(max/2)})
    ));
    assert!(!properties_fit(
        &serde_json::json!({"x":vec![serde_json::Value::Null;4096]})
    ));
    let mut deep = serde_json::Value::Null;
    for _ in 0..66 {
        deep = serde_json::json!([deep]);
    }
    assert!(!properties_fit(&deep));
    assert!(
        QuarantineMessage::new(
            vec![0; QuarantineMessage::MAX_BODY_BYTES],
            exact,
            None,
            QuarantineReason::Unsupported
        )
        .is_some()
    );
    assert!(
        QuarantineMessage::new(
            vec![0; QuarantineMessage::MAX_BODY_BYTES + 1],
            serde_json::json!({}),
            None,
            QuarantineReason::Unsupported
        )
        .is_none()
    );
}
