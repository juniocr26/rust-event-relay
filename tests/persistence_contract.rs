use std::{error::Error, io, sync::Mutex};

use chrono::{DateTime, Utc};
use reliable_event_relay::{
    domain::event::EventEnvelope,
    persistence::{
        BatchSize, EligibleRead, InvalidBatchSize, OutboxReader, PendingOutboxEvent,
        PersistenceError, PersistenceErrorKind,
    },
};
use serde_json::json;

fn instant() -> DateTime<Utc> {
    "2026-10-07T18:00:00Z".parse().unwrap()
}

fn snapshot() -> PendingOutboxEvent {
    let event: EventEnvelope = serde_json::from_value(json!({
        "id": "0199ba40-0000-7000-8000-000000000001",
        "event_type": "order.created",
        "aggregate_type": "order",
        "aggregate_id": "external:42",
        "schema_version": 4294967295_u32,
        "occurred_at": "2026-10-07T12:00:00Z",
        "correlation_id": "0199ba40-0000-7000-8000-000000000002",
        "causation_id": null,
        "payload": {"example": true}
    }))
    .unwrap();
    PendingOutboxEvent::new(event, 2, instant())
}

#[test]
fn bounded_requests_reject_zero_without_an_arbitrary_maximum() {
    assert_eq!(BatchSize::new(0), Err(InvalidBatchSize));
    for value in [1, 32, usize::MAX] {
        let batch = BatchSize::new(value).unwrap();
        let request = EligibleRead::new(instant(), batch);
        assert_eq!(request.limit().get(), value);
        assert_eq!(request.eligible_at(), instant());
    }
}

#[test]
fn pending_snapshot_preserves_canonical_identity_and_keeps_metadata_separate() {
    let record = snapshot();
    assert_eq!(record.attempt_count(), 2);
    assert_eq!(record.available_at(), instant());
    let original = record.event().clone();
    let wrapped = PendingOutboxEvent::new(original.clone(), 0, instant());
    assert_eq!(wrapped.event(), &original);
    assert_eq!(wrapped.event().schema_version(), u32::MAX);
    assert_eq!(
        wrapped.event().occurred_at(),
        "2026-10-07T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    let encoded = serde_json::to_value(wrapped.event()).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 9);
    assert!(encoded.get("attempt_count").is_none());
    assert!(encoded.get("available_at").is_none());
}

#[test]
fn persistence_errors_keep_classification_and_sources_but_redact_formatting() {
    let secret = "synthetic private connection and payload details";
    for kind in [
        PersistenceErrorKind::Unavailable,
        PersistenceErrorKind::InvalidStoredData,
        PersistenceErrorKind::OperationFailed,
    ] {
        let without_source = PersistenceError::new(kind);
        assert_eq!(without_source.kind(), kind);
        assert!(without_source.source().is_none());
        let error = PersistenceError::with_source(kind, io::Error::other(secret));
        assert_eq!(error.kind(), kind);
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
        let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(source.to_string(), secret);
    }
}

// One scripted response plus request capture, not an alternate persistence engine.
struct ScriptedReader {
    requests: Mutex<Vec<EligibleRead>>,
    response: Mutex<Option<Result<Vec<PendingOutboxEvent>, PersistenceError>>>,
}

impl ScriptedReader {
    fn new(response: Result<Vec<PendingOutboxEvent>, PersistenceError>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            response: Mutex::new(Some(response)),
        }
    }
}

impl OutboxReader for ScriptedReader {
    async fn read_eligible(
        &self,
        request: EligibleRead,
    ) -> Result<Vec<PendingOutboxEvent>, PersistenceError> {
        // A real await makes Send part of the exercised contract, not just syntax.
        tokio::task::yield_now().await;
        self.requests.lock().unwrap().push(request);
        self.response.lock().unwrap().take().unwrap()
    }
}

async fn read_through_port<R: OutboxReader>(
    reader: &R,
    request: EligibleRead,
) -> Result<Vec<PendingOutboxEvent>, PersistenceError> {
    reader.read_eligible(request).await
}

#[tokio::test]
async fn generic_caller_uses_bounded_contract_on_a_send_future() {
    let request = EligibleRead::new(instant(), BatchSize::new(1).unwrap());
    let expected = snapshot();
    let reader = ScriptedReader::new(Ok(vec![expected.clone()]));
    let result = tokio::spawn(async move {
        let result = read_through_port(&reader, request).await.unwrap();
        assert_eq!(*reader.requests.lock().unwrap(), vec![request]);
        result
    })
    .await
    .unwrap();
    assert_eq!(result, vec![expected]);
}

#[tokio::test]
async fn generic_caller_handles_empty_snapshot_and_classified_failure() {
    let request = EligibleRead::new(instant(), BatchSize::new(4).unwrap());
    let empty = ScriptedReader::new(Ok(Vec::new()));
    assert!(read_through_port(&empty, request).await.unwrap().is_empty());
    let unavailable = ScriptedReader::new(Err(PersistenceError::with_source(
        PersistenceErrorKind::Unavailable,
        io::Error::other("synthetic unavailable source"),
    )));
    let error = read_through_port(&unavailable, request).await.unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::Unavailable);
    assert_eq!(
        error.source().unwrap().to_string(),
        "synthetic unavailable source"
    );
}
