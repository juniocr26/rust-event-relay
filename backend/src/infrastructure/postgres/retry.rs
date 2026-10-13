//! Exact-row retry transactions, separate from application policy and publication.
use super::{
    PostgresOutboxRepository, StoredTimestamp, invalid_data, map_driver_error,
    ownership::{begin, clock, map_commit_error, retry_state},
};
use crate::{
    domain::retry::{FailureEvidence, FailurePhase, RetryError, TerminalReason},
    persistence::*,
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

fn operation(error: RetryError) -> PersistenceError {
    PersistenceError::with_source(PersistenceErrorKind::OperationFailed, error)
}
async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    maintenance: bool,
) -> Result<Option<PgRow>, PersistenceError> {
    let sql = if maintenance {
        "SELECT status,ownership_token,acquired_at,expires_at,available_at,attempt_count,retry_cycle,cycle_baseline,cycle_maximum,state_revision FROM relay.outbox_events WHERE id=$1 FOR UPDATE SKIP LOCKED"
    } else {
        "SELECT status,ownership_token,acquired_at,expires_at,available_at,attempt_count,retry_cycle,cycle_baseline,cycle_maximum,state_revision FROM relay.outbox_events WHERE id=$1 FOR UPDATE"
    };
    sqlx::query(sql)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_driver_error)
}
fn time(row: &PgRow, column: &str) -> Result<Option<DateTime<Utc>>, PersistenceError> {
    Ok(row
        .try_get::<Option<StoredTimestamp>, _>(column)
        .map_err(invalid_data)?
        .map(|t| t.0))
}
fn owned(row: &PgRow, owner: RetryOwner, now: DateTime<Utc>) -> Result<bool, PersistenceError> {
    let status: String = row.try_get("status").map_err(invalid_data)?;
    let token: Option<Uuid> = row.try_get("ownership_token").map_err(invalid_data)?;
    let cycle: i32 = row.try_get("retry_cycle").map_err(invalid_data)?;
    let revision: i64 = row.try_get("state_revision").map_err(invalid_data)?;
    Ok(status == "pending"
        && token == Some(owner.key().token().get())
        && cycle == owner.cycle()
        && revision == owner.revision()
        && time(row, "acquired_at")?.is_some_and(|t| t <= now)
        && time(row, "expires_at")?.is_some_and(|t| now < t))
}
fn lifetime(row: &PgRow) -> Result<u32, PersistenceError> {
    u32::try_from(
        row.try_get::<i32, _>("attempt_count")
            .map_err(invalid_data)?,
    )
    .map_err(invalid_data)
}
async fn conflict<T>(
    tx: Transaction<'_, Postgres>,
) -> Result<RetryTransition<T>, PersistenceError> {
    tx.rollback().await.map_err(map_driver_error)?;
    Ok(RetryTransition::Conflict)
}
impl OutboxRetryScheduler for PostgresOutboxRepository {
    async fn schedule_retry(
        &self,
        request: ScheduleRetry,
    ) -> Result<RetryTransition<ScheduledRetry>, PersistenceError> {
        let mut tx = begin(self).await?;
        let Some(row) = lock(&mut tx, request.owner.key().event_id(), false).await? else {
            return conflict(tx).await;
        };
        let now = clock(&mut tx).await?;
        if !owned(&row, request.owner, now)? {
            return conflict(tx).await;
        }
        let count = lifetime(&row)?;
        let state = retry_state(&row, count)?;
        if !state.can_acquire(count).map_err(invalid_data)? {
            return conflict(tx).await;
        }
        let available_at = request.delay.available_at(now).map_err(operation)?;
        let revision = state.next_revision().map_err(operation)?;
        let result=sqlx::query("UPDATE relay.outbox_events SET available_at=$2,diagnostic_code=$3,diagnostic_at=$4,failure_evidence=$5,ownership_token=NULL,acquired_at=NULL,expires_at=NULL,state_revision=$6 WHERE id=$1 AND status='pending' AND state_revision=$7 AND retry_cycle=$8 AND ownership_token=$9")
   .bind(request.owner.key().event_id()).bind(available_at).bind(request.diagnostic.as_str()).bind(now).bind(request.evidence.to_json()).bind(revision).bind(request.owner.revision()).bind(request.owner.cycle()).bind(request.owner.key().token().get())
   .execute(&mut *tx).await.map_err(map_driver_error)?;
        if result.rows_affected() != 1 {
            return conflict(tx).await;
        }
        tx.commit().await.map_err(map_commit_error)?;
        Ok(RetryTransition::Applied(ScheduledRetry {
            available_at,
            revision,
        }))
    }
}
pub(super) async fn terminal(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
    reason: TerminalReason,
    evidence: FailureEvidence,
    revision: i64,
) -> Result<(), PersistenceError> {
    let result=sqlx::query("UPDATE relay.outbox_events SET status='dead_letter',processed_at=NULL,terminal_at=$2,terminal_reason=$3,diagnostic_code=$3,diagnostic_at=$2,failure_evidence=$4,ownership_token=NULL,acquired_at=NULL,expires_at=NULL,state_revision=$5 WHERE id=$1 AND status='pending' AND state_revision=$6")
  .bind(id).bind(now).bind(reason.as_str()).bind(evidence.to_json()).bind(revision).bind(revision.checked_sub(1).ok_or_else(|| operation(RetryError::RevisionLimit))?).execute(&mut **tx).await.map_err(map_driver_error)?;
    if result.rows_affected() != 1 {
        return Err(PersistenceError::new(PersistenceErrorKind::OperationFailed));
    }
    Ok(())
}
impl OutboxFailurePreserver for PostgresOutboxRepository {
    async fn preserve_failure(
        &self,
        request: PreserveFailure,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        let mut tx = begin(self).await?;
        let Some(row) = lock(&mut tx, request.owner.key().event_id(), false).await? else {
            return conflict(tx).await;
        };
        let now = clock(&mut tx).await?;
        if !owned(&row, request.owner, now)? {
            return conflict(tx).await;
        }
        let count = lifetime(&row)?;
        let state = retry_state(&row, count)?;
        // Budget reasons must have matching durable evidence; permanence is caller authority.
        if (request.reason == TerminalReason::StorageCounterExhausted && count != i32::MAX as u32)
            || (request.reason == TerminalReason::AcquisitionBudgetExhausted
                && state.consumed(count).map_err(invalid_data)? < state.maximum() as u32)
        {
            return conflict(tx).await;
        }
        let revision = state.next_revision().map_err(operation)?;
        terminal(
            &mut tx,
            request.owner.key().event_id(),
            now,
            request.reason,
            request.evidence,
            revision,
        )
        .await?;
        tx.commit().await.map_err(map_commit_error)?;
        Ok(RetryTransition::Applied(PreservedFailure {
            terminal_at: now,
            reason: request.reason,
            revision,
        }))
    }
}
impl OutboxExhaustedMaintainer for PostgresOutboxRepository {
    async fn preserve_exhausted(
        &self,
        request: ExhaustedEvent,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        let mut tx = begin(self).await?;
        let Some(row) = lock(&mut tx, request.id(), true).await? else {
            return conflict(tx).await;
        };
        let now = clock(&mut tx).await?;
        let status: String = row.try_get("status").map_err(invalid_data)?;
        let count = lifetime(&row)?;
        let state = retry_state(&row, count)?;
        if status != "pending"
            || state.cycle() != request.cycle()
            || state.revision() != request.revision()
            || time(&row, "available_at")?.is_none_or(|t| t > now)
            || time(&row, "expires_at")?.is_some_and(|t| t > now)
        {
            return conflict(tx).await;
        }
        if super::discovery::maintenance_eligibility(&row, now, count)? != CandidateEvidence::Valid
        {
            return conflict(tx).await;
        }
        let reason = if count == i32::MAX as u32 {
            TerminalReason::StorageCounterExhausted
        } else if state.consumed(count).map_err(invalid_data)? >= state.maximum() as u32 {
            TerminalReason::AcquisitionBudgetExhausted
        } else {
            return conflict(tx).await;
        };
        let revision = state.next_revision().map_err(operation)?;
        let evidence =
            FailureEvidence::new(FailurePhase::Maintenance, false, None).map_err(operation)?;
        terminal(&mut tx, request.id(), now, reason, evidence, revision).await?;
        tx.commit().await.map_err(map_commit_error)?;
        Ok(RetryTransition::Applied(PreservedFailure {
            terminal_at: now,
            reason,
            revision,
        }))
    }
}
