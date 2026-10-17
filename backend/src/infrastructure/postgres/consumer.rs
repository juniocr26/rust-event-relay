//! Atomic inventory persistence / Persistência atômica de estoque.
//! READ COMMITTED; identity insertion precedes stock locking. No retries or broker I/O.
use super::{StoredTimestamp, invalid_data, map_driver_error, ownership::map_commit_error};
use crate::{
    application::consumer::{
        AtomicInventoryProcessor, EffectReceipt, ProcessingFailure, ProcessingFailureKind,
        ProcessingOutcome,
    },
    domain::{
        consumer::{ConsumerName, ContentMatch, InvalidEvent, InventoryEvent, Stock},
        event::EventEnvelope,
    },
};
use sqlx::{PgPool, Row, postgres::PgRow};

const MAX_CONTENT: usize = 16384;

/// Injected pool with caller-configured acquire/statement/lock deadlines.
/// Pool injetado com prazos de aquisição, comandos e locks definidos pelo chamador.
#[derive(Clone)]
pub struct PostgresInventoryProcessor {
    pool: PgPool,
    demo_context: Option<(Option<uuid::Uuid>, &'static str)>,
}
impl PostgresInventoryProcessor {
    /// Construction performs no I/O; the caller owns pool lifecycle/deadlines.
    /// Construção sem I/O; o chamador controla ciclo de vida e prazos do pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            demo_context: None,
        }
    }
    /// Durable classification only for demo-owned events; legacy processing stays unchanged.
    /// Classificação durável apenas para eventos demo; preserva processamento anterior.
    pub fn with_demo_evidence(mut self) -> Self {
        self.demo_context = Some((None, "main"));
        self
    }
    pub fn for_demo_action(mut self, id: uuid::Uuid, slot: &'static str) -> Self {
        self.demo_context = Some((Some(id), slot));
        self
    }
    async fn observe_demo(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &InventoryEvent,
        outcome: &str,
    ) -> Result<(), ProcessingFailure> {
        let Some((action, context)) = self.demo_context else {
            return Ok(());
        };
        let run: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT run_id FROM demo.runs WHERE event_id=$1 FOR UPDATE")
                .bind(event.event_id())
                .fetch_optional(&mut **tx)
                .await
                .map_err(precommit)?;
        if let Some(run) = run {
            if let Some(action) = action {
                let valid: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM demo.actions WHERE action_id=$1 AND run_id=$2)",
                )
                .bind(action)
                .bind(run)
                .fetch_one(&mut **tx)
                .await
                .map_err(precommit)?;
                if !valid {
                    return Err(ProcessingFailure::new(
                        ProcessingFailureKind::InvalidStoredData,
                    ));
                }
            }
            sqlx::query("INSERT INTO demo.consumer_observations(observation_id,run_id,event_id,action_id,context,outcome) SELECT $1,$2,$3,$4,$5,$6 WHERE (SELECT count(*) FROM demo.consumer_observations WHERE run_id=$2)<32").bind(uuid::Uuid::now_v7()).bind(run).bind(event.event_id()).bind(action).bind(context).bind(outcome).execute(&mut **tx).await.map_err(precommit)?;
        }
        Ok(())
    }
}
fn precommit(error: sqlx::Error) -> ProcessingFailure {
    map_driver_error(error).into()
}
fn stored(error: impl std::error::Error + Send + Sync + 'static) -> ProcessingFailure {
    invalid_data(error).into()
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
        return Err(ProcessingFailure::new(
            ProcessingFailureKind::InvalidStoredData,
        ));
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
        return Err(ProcessingFailure::new(
            ProcessingFailureKind::InvalidStoredData,
        ));
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
            if matches!(outcome, ProcessingOutcome::Duplicate(_)) && self.demo_context.is_some() {
                self.observe_demo(&mut tx, event, "Duplicate").await?;
                tx.commit()
                    .await
                    .map_err(|error| ProcessingFailure::from(map_commit_error(error)))?;
            } else {
                tx.rollback().await.map_err(precommit)?;
            }
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
        self.observe_demo(&mut tx, event, "Applied").await?;
        tx.commit()
            .await
            .map_err(|error| ProcessingFailure::from(map_commit_error(error)))?;
        Ok(ProcessingOutcome::Applied(EffectReceipt {
            stock_after: after,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn failure_sources_are_typed_and_public_formatting_is_sanitized() {
        let secret = "private payload and connection details";
        let error = precommit(sqlx::Error::Io(std::io::Error::other(secret)));
        assert_eq!(error.kind(), ProcessingFailureKind::Unavailable);
        assert!(error.source().unwrap().is::<sqlx::Error>());
        assert!(!format!("{error} {error:?}").contains(secret));
        let error = stored(serde_json::from_slice::<EventEnvelope>(b"invalid").unwrap_err());
        assert_eq!(error.kind(), ProcessingFailureKind::InvalidStoredData);
        assert!(error.source().unwrap().is::<serde_json::Error>());
        for (version, bytes) in [(2, b"{}".as_slice()), (1, b""), (1, b"invalid")] {
            assert_eq!(
                decode(version, bytes).unwrap_err().kind(),
                ProcessingFailureKind::InvalidStoredData
            );
        }
        let error = ProcessingFailure::from(map_commit_error(sqlx::Error::Protocol(secret.into())));
        assert_eq!(error.kind(), ProcessingFailureKind::CommitUncertain);
        assert!(error.source().unwrap().is::<sqlx::Error>());
        assert!(!format!("{error} {error:?}").contains(secret));
    }
}
