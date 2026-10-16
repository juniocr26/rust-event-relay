//! Atomic inventory persistence / Persistência atômica de estoque.
//! READ COMMITTED; identity insertion precedes stock locking. No retries or broker I/O.
use super::{StoredTimestamp, ownership::map_commit_error};
use crate::{
    application::consumer::{
        AtomicInventoryProcessor, EffectReceipt, ProcessingFailure, ProcessingOutcome,
    },
    domain::{
        consumer::{ConsumerName, ContentMatch, InvalidEvent, InventoryEvent, Stock},
        event::EventEnvelope,
    },
    persistence::PersistenceErrorKind,
};
use sqlx::{PgPool, Row, postgres::PgRow};

const MAX_CONTENT: usize = 16384;

/// Injected pool with caller-configured acquire/statement/lock deadlines.
/// Pool injetado com prazos de aquisição, comandos e locks definidos pelo chamador.
#[derive(Clone)]
pub struct PostgresInventoryProcessor {
    pool: PgPool,
}
impl PostgresInventoryProcessor {
    /// Construction performs no I/O; the caller owns pool lifecycle/deadlines.
    /// Construção sem I/O; o chamador controla ciclo de vida e prazos do pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn precommit(_: sqlx::Error) -> ProcessingFailure {
    ProcessingFailure::PreCommit
}
fn stored(_: impl std::fmt::Debug) -> ProcessingFailure {
    ProcessingFailure::InvalidStoredData
}

// BYTEA avoids JSONB number normalization and timestamp microsecond truncation.
// Equality is checked model equality, never serialized-byte equality or a digest.
fn encode(event: &InventoryEvent) -> Result<Vec<u8>, InvalidEvent> {
    let bytes = serde_json::to_vec(event.envelope()).map_err(|_| InvalidEvent::Unsupported)?;
    if bytes.len() > MAX_CONTENT {
        return Err(InvalidEvent::Unsupported);
    }
    let restored = decode(1, &bytes).map_err(|_| InvalidEvent::Unsupported)?;
    if restored != *event {
        return Err(InvalidEvent::Unsupported);
    }
    Ok(bytes)
}
fn decode(version: i16, bytes: &[u8]) -> Result<InventoryEvent, ProcessingFailure> {
    if version != 1 || bytes.is_empty() || bytes.len() > MAX_CONTENT {
        return Err(ProcessingFailure::InvalidStoredData);
    }
    let envelope: EventEnvelope = serde_json::from_slice(bytes).map_err(stored)?;
    let id = envelope.id().to_string();
    InventoryEvent::validate(envelope, Some(&id)).map_err(stored)
}
fn receipt(
    row: &PgRow,
    id: uuid::Uuid,
) -> Result<(InventoryEvent, EffectReceipt), ProcessingFailure> {
    let event = decode(
        row.try_get("representation_version").map_err(stored)?,
        &row.try_get::<Vec<u8>, _>("immutable_content")
            .map_err(stored)?,
    )?;
    let before = Stock::new(row.try_get("stock_before").map_err(stored)?).map_err(stored)?;
    let after = Stock::new(row.try_get("stock_after").map_err(stored)?).map_err(stored)?;
    let _: StoredTimestamp = row.try_get("completed_at").map_err(stored)?;
    let warehouse: String = row.try_get("warehouse_id").map_err(stored)?;
    let sku: String = row.try_get("sku").map_err(stored)?;
    let quantity: i64 = row.try_get("quantity").map_err(stored)?;
    if event.event_id() != id
        || event.warehouse_id() != warehouse
        || event.sku() != sku
        || event.quantity() != quantity
        || before.receive(&event).map_err(stored)? != after
    {
        return Err(ProcessingFailure::InvalidStoredData);
    }
    Ok((event, EffectReceipt { stock_after: after }))
}
impl AtomicInventoryProcessor for PostgresInventoryProcessor {
    async fn process(
        &self,
        consumer: &ConsumerName,
        event: &InventoryEvent,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        let content = match encode(event) {
            Ok(bytes) => bytes,
            Err(error) => return Ok(ProcessingOutcome::Invalid(error)),
        };
        let mut tx = self.pool.begin().await.map_err(precommit)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await
            .map_err(precommit)?;
        let inserted = sqlx::query("INSERT INTO consumer.receipts(consumer_name,event_id,representation_version,immutable_content,warehouse_id,sku,quantity) VALUES($1,$2,1,$3,$4,$5,$6) ON CONFLICT(consumer_name,event_id) DO NOTHING RETURNING event_id")
            .bind(consumer.as_str()).bind(event.event_id()).bind(content).bind(event.warehouse_id()).bind(event.sku()).bind(event.quantity()).fetch_optional(&mut *tx).await.map_err(precommit)?;
        if inserted.is_none() {
            // Fresh statement snapshot sees the winner after ON CONFLICT waited.
            let row = sqlx::query(
                "SELECT * FROM consumer.receipts WHERE consumer_name=$1 AND event_id=$2",
            )
            .bind(consumer.as_str())
            .bind(event.event_id())
            .fetch_one(&mut *tx)
            .await
            .map_err(precommit)?;
            let (prior, receipt) = receipt(&row, event.event_id())?;
            let outcome = match event.compare(&prior) {
                ContentMatch::Duplicate => ProcessingOutcome::Duplicate(receipt),
                _ => ProcessingOutcome::IdentityConflict,
            };
            tx.rollback().await.map_err(precommit)?;
            return Ok(outcome);
        }
        sqlx::query("INSERT INTO consumer.stock(warehouse_id,sku,units) VALUES($1,$2,0) ON CONFLICT DO NOTHING")
            .bind(event.warehouse_id()).bind(event.sku()).execute(&mut *tx).await.map_err(precommit)?;
        let units: i64 = sqlx::query_scalar(
            "SELECT units FROM consumer.stock WHERE warehouse_id=$1 AND sku=$2 FOR UPDATE",
        )
        .bind(event.warehouse_id())
        .bind(event.sku())
        .fetch_one(&mut *tx)
        .await
        .map_err(precommit)?;
        let before = Stock::new(units).map_err(stored)?;
        let after = match before.receive(event) {
            Ok(after) => after,
            Err(error) => {
                tx.rollback().await.map_err(precommit)?;
                return Ok(ProcessingOutcome::Invalid(error));
            }
        };
        sqlx::query("UPDATE consumer.stock SET units=$3 WHERE warehouse_id=$1 AND sku=$2")
            .bind(event.warehouse_id())
            .bind(event.sku())
            .bind(after.units())
            .execute(&mut *tx)
            .await
            .map_err(precommit)?;
        sqlx::query("UPDATE consumer.receipts SET stock_before=$3,stock_after=$4,completed_at=clock_timestamp() WHERE consumer_name=$1 AND event_id=$2")
            .bind(consumer.as_str()).bind(event.event_id()).bind(before.units()).bind(after.units()).execute(&mut *tx).await.map_err(precommit)?;
        tx.commit().await.map_err(|error| {
            if map_commit_error(error).kind() == PersistenceErrorKind::CommitUncertain {
                ProcessingFailure::CommitUncertain
            } else {
                ProcessingFailure::PreCommit
            }
        })?;
        Ok(ProcessingOutcome::Applied(EffectReceipt {
            stock_after: after,
        }))
    }
}
