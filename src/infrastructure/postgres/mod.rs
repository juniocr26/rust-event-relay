//! PostgreSQL read-only persistence. Pool lifecycle belongs to the caller.
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{
    Decode, PgPool, Postgres, Row, Type,
    postgres::{PgRow, PgTypeInfo, PgValueFormat, PgValueRef},
};
use uuid::Uuid;

use crate::{
    domain::event::{EventEnvelope, EventEnvelopeParts, EventType},
    persistence::{
        EligibleRead, OutboxReader, PendingOutboxEvent, PersistenceError, PersistenceErrorKind,
    },
};

const READ_ELIGIBLE: &str = "SELECT id, event_type, aggregate_type, aggregate_id, schema_version, \
    occurred_at, correlation_id, causation_id, payload, attempt_count, available_at \
    FROM relay.outbox_events WHERE status = 'pending' AND available_at <= $1 \
    ORDER BY available_at, created_at, id LIMIT $2";

/// Focused reader backed by an injected pool; constructing it does not connect.
#[derive(Clone)]
pub struct PostgresOutboxRepository {
    pool: PgPool,
}

impl PostgresOutboxRepository {
    /// The composition boundary configures, connects and closes the pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl OutboxReader for PostgresOutboxRepository {
    async fn read_eligible(
        &self,
        request: EligibleRead,
    ) -> Result<Vec<PendingOutboxEvent>, PersistenceError> {
        let limit = postgres_limit(request.limit().get())?;
        let rows = sqlx::query(READ_ELIGIBLE)
            .bind(request.eligible_at())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(map_driver_error)?;
        rows.iter()
            .map(|row| StoredEvent::decode(row)?.restore())
            .collect()
    }
}

// PostgreSQL LIMIT accepts signed BIGINT. Oversized requests fail before I/O.
fn postgres_limit(value: usize) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|error| {
        PersistenceError::with_source(PersistenceErrorKind::OperationFailed, error)
    })
}

// SQLx 0.8.6's Chrono decoder uses unchecked timestamp arithmetic.
// Decode privately with checked arithmetic, including PostgreSQL infinity sentinels.
struct StoredTimestamp(DateTime<Utc>);

impl Type<Postgres> for StoredTimestamp {
    fn type_info() -> PgTypeInfo {
        <DateTime<Utc> as Type<Postgres>>::type_info()
    }
}

fn timestamp_from_micros(micros: i64) -> Result<DateTime<Utc>, std::io::Error> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unsupported stored timestamp",
        )
    };
    if matches!(micros, i64::MIN | i64::MAX) {
        return Err(invalid());
    }
    DateTime::<Utc>::from_timestamp(946_684_800, 0)
        .and_then(|epoch| epoch.checked_add_signed(chrono::Duration::microseconds(micros)))
        .ok_or_else(invalid)
}

impl<'r> Decode<'r, Postgres> for StoredTimestamp {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let timestamp = match value.format() {
            PgValueFormat::Binary => {
                let bytes: [u8; 8] = value.as_bytes()?.try_into()?;
                timestamp_from_micros(i64::from_be_bytes(bytes))?
            }
            PgValueFormat::Text => {
                DateTime::parse_from_str(value.as_str()?, "%Y-%m-%d %H:%M:%S%.f%#z")?
                    .with_timezone(&Utc)
            }
        };
        Ok(Self(timestamp))
    }
}

// Preserve JSONB numeric values exactly, and reject unknown binary versions.
struct StoredJson(Value);

impl Type<Postgres> for StoredJson {
    fn type_info() -> PgTypeInfo {
        <Value as Type<Postgres>>::type_info()
    }
}

fn decode_json(bytes: &[u8], binary: bool) -> Result<Value, sqlx::error::BoxDynError> {
    let json = if binary {
        match bytes.split_first() {
            Some((&1, json)) => json,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unsupported stored JSONB representation",
                )
                .into());
            }
        }
    } else {
        bytes
    };
    Ok(serde_json::from_slice(json)?)
}

impl<'r> Decode<'r, Postgres> for StoredJson {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self(decode_json(
            value.as_bytes()?,
            value.format() == PgValueFormat::Binary,
        )?))
    }
}

// Private row data never crosses the infrastructure boundary.
struct StoredEvent {
    id: Uuid,
    event_type: String,
    aggregate_type: String,
    aggregate_id: String,
    schema_version: i64,
    occurred_at: DateTime<Utc>,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    payload: Value,
    attempt_count: i32,
    available_at: DateTime<Utc>,
}

impl StoredEvent {
    fn decode(row: &PgRow) -> Result<Self, PersistenceError> {
        let decode = || -> Result<Self, sqlx::Error> {
            Ok(Self {
                id: row.try_get("id")?,
                event_type: row.try_get("event_type")?,
                aggregate_type: row.try_get("aggregate_type")?,
                aggregate_id: row.try_get("aggregate_id")?,
                schema_version: row.try_get("schema_version")?,
                occurred_at: row.try_get::<StoredTimestamp, _>("occurred_at")?.0,
                correlation_id: row.try_get("correlation_id")?,
                causation_id: row.try_get("causation_id")?,
                payload: row.try_get::<StoredJson, _>("payload")?.0,
                attempt_count: row.try_get("attempt_count")?,
                available_at: row.try_get::<StoredTimestamp, _>("available_at")?.0,
            })
        };
        decode().map_err(invalid_data)
    }

    fn restore(self) -> Result<PendingOutboxEvent, PersistenceError> {
        let version = u32::try_from(self.schema_version).map_err(invalid_data)?;
        let attempts = u32::try_from(self.attempt_count).map_err(invalid_data)?;
        let event = EventEnvelope::restore(EventEnvelopeParts {
            id: self.id,
            event_type: EventType::new(self.event_type).map_err(invalid_data)?,
            aggregate_type: self.aggregate_type,
            aggregate_id: self.aggregate_id,
            schema_version: version,
            occurred_at: self.occurred_at,
            correlation_id: self.correlation_id,
            causation_id: self.causation_id,
            payload: self.payload,
        })
        .map_err(invalid_data)?;
        Ok(PendingOutboxEvent::new(event, attempts, self.available_at))
    }
}

fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> PersistenceError {
    PersistenceError::with_source(PersistenceErrorKind::InvalidStoredData, error)
}

fn map_driver_error(error: sqlx::Error) -> PersistenceError {
    use PersistenceErrorKind::{InvalidStoredData, OperationFailed, Unavailable};
    let kind = match &error {
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => Unavailable,
        sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_) => InvalidStoredData,
        sqlx::Error::Database(database) => classify_sqlstate(database.code().as_deref()),
        _ => OperationFailed,
    };
    PersistenceError::with_source(kind, error)
}

fn classify_sqlstate(code: Option<&str>) -> PersistenceErrorKind {
    match code {
        Some(code)
            if code.starts_with("08")
                || code.starts_with("28")
                || matches!(code, "53300" | "57P01" | "57P02" | "57P03") =>
        {
            PersistenceErrorKind::Unavailable
        }
        _ => PersistenceErrorKind::OperationFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn stored() -> StoredEvent {
        StoredEvent {
            id: Uuid::from_u128(1),
            event_type: " event ".into(),
            aggregate_type: " aggregate ".into(),
            aggregate_id: " id ".into(),
            schema_version: i64::from(u32::MAX),
            occurred_at: DateTime::from_timestamp(123, 456_000).unwrap(),
            correlation_id: Some(Uuid::from_u128(2)),
            causation_id: Some(Uuid::from_u128(3)),
            payload: serde_json::json!({"nested": [null, 42, "value"]}),
            attempt_count: i32::MAX,
            available_at: DateTime::from_timestamp(789, 0).unwrap(),
        }
    }

    #[test]
    fn faithful_restoration() {
        let original = stored();
        let expected = serde_json::json!({"id": original.id, "event_type": original.event_type,
            "aggregate_type": original.aggregate_type, "aggregate_id": original.aggregate_id,
            "schema_version": original.schema_version, "occurred_at": original.occurred_at,
            "correlation_id": original.correlation_id, "causation_id": original.causation_id, "payload": original.payload});
        let restored = original.restore().unwrap();
        assert_eq!(serde_json::to_value(restored.event()).unwrap(), expected);
        assert_eq!(restored.attempt_count(), u32::try_from(i32::MAX).unwrap());
        assert_eq!(
            restored.available_at(),
            DateTime::from_timestamp(789, 0).unwrap()
        );
        let mut null = stored();
        null.payload = Value::Null;
        assert_eq!(null.restore().unwrap().event().payload(), &Value::Null);
    }

    #[test]
    fn rejects_malformed_values_without_partial_results() {
        for version in [-1, 0, i64::from(u32::MAX) + 1] {
            let mut row = stored();
            row.schema_version = version;
            assert_eq!(
                row.restore().unwrap_err().kind(),
                PersistenceErrorKind::InvalidStoredData
            );
        }
        for field in 0..4 {
            let mut row = stored();
            match field {
                0 => row.event_type = " \t".into(),
                1 => row.aggregate_type = " ".into(),
                2 => row.aggregate_id = "".into(),
                _ => row.attempt_count = -1,
            }
            let error = row.restore().unwrap_err();
            assert_eq!(error.kind(), PersistenceErrorKind::InvalidStoredData);
            assert!(error.source().is_some());
        }
        let mut bad = stored();
        bad.attempt_count = -1;
        let result: Result<Vec<_>, _> = [stored(), bad]
            .into_iter()
            .map(StoredEvent::restore)
            .collect();
        assert!(result.is_err());
    }

    #[test]
    fn sqlstate_classification_is_conservative() {
        for code in ["08006", "28P01", "53300", "57P01", "57P02", "57P03"] {
            assert_eq!(
                classify_sqlstate(Some(code)),
                PersistenceErrorKind::Unavailable
            );
        }
        for code in [
            Some("42501"),
            Some("42P01"),
            Some("40001"),
            Some("57014"),
            Some("unknown"),
            None,
        ] {
            assert_eq!(
                classify_sqlstate(code),
                PersistenceErrorKind::OperationFailed
            );
        }
    }

    #[test]
    fn timestamps_reject_infinity_and_chrono_overflow() {
        for micros in [i64::MIN, i64::MAX, i64::MAX - 1] {
            assert!(timestamp_from_micros(micros).is_err());
        }
        assert_eq!(
            timestamp_from_micros(0).unwrap(),
            DateTime::from_timestamp(946_684_800, 0).unwrap()
        );
        assert_eq!(
            timestamp_from_micros(-1).unwrap(),
            DateTime::from_timestamp(946_684_799, 999_999_000).unwrap()
        );
    }

    #[test]
    fn json_preserves_precision_and_rejects_unsupported_representations() {
        let text = br#"{"integer":18446744073709551617,"decimal":0.12345678901234567890123456789,"huge":1e1000}"#;
        let restored = decode_json(text, false).unwrap();
        assert_eq!(restored["integer"].to_string(), "18446744073709551617");
        assert_eq!(
            restored["decimal"].to_string(),
            "0.12345678901234567890123456789"
        );
        assert_eq!(decode_json(b"\x01null", true).unwrap(), Value::Null);
        for bytes in [b"".as_slice(), b"\x02null", b"\x01{invalid"] {
            let source = decode_json(bytes, true).unwrap_err();
            let error = PersistenceError::with_source(
                PersistenceErrorKind::InvalidStoredData,
                sqlx::Error::Decode(source),
            );
            assert_eq!(error.kind(), PersistenceErrorKind::InvalidStoredData);
            assert!(error.source().is_some());
        }
    }

    #[test]
    fn checked_limit() {
        assert_eq!(postgres_limit(1).unwrap(), 1);
        if let Ok(max) = usize::try_from(i64::MAX) {
            assert_eq!(postgres_limit(max).unwrap(), i64::MAX);
            let error = postgres_limit(max + 1).unwrap_err();
            assert_eq!(error.kind(), PersistenceErrorKind::OperationFailed);
            assert!(error.source().unwrap().is::<std::num::TryFromIntError>());
        }
    }

    #[test]
    fn driver_classification_and_sanitized_sources() {
        let cases = [
            (sqlx::Error::PoolClosed, PersistenceErrorKind::Unavailable),
            (sqlx::Error::PoolTimedOut, PersistenceErrorKind::Unavailable),
            (
                sqlx::Error::Io(std::io::Error::other("secret payload credentials")),
                PersistenceErrorKind::Unavailable,
            ),
            (
                sqlx::Error::Decode(Box::new(std::io::Error::other(
                    "secret payload credentials",
                ))),
                PersistenceErrorKind::InvalidStoredData,
            ),
            (
                sqlx::Error::Protocol("secret payload credentials".into()),
                PersistenceErrorKind::OperationFailed,
            ),
        ];
        for (source, kind) in cases {
            let error = map_driver_error(source);
            assert_eq!(error.kind(), kind);
            assert!(error.source().unwrap().is::<sqlx::Error>());
            assert!(!format!("{error} {error:?}").contains("secret"));
        }
    }
}
