//! Short, Read Committed transactions. No broker I/O or hidden retries.
use super::{
    PostgresOutboxRepository, StoredEvent, StoredTimestamp, invalid_data, map_driver_error,
};
use crate::domain::retry::RetryState;
use crate::{
    domain::delivery::DeliveryLease,
    persistence::{
        AcquireRequest, OutboxAcquirer, OutboxCompleter, OutboxReleaser, OwnedEventKey,
        OwnedOutboxEvent, PendingOutboxEvent, PersistenceError, PersistenceErrorKind,
        TransitionOutcome,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Row, Transaction};

// The first clock sample is a selection cutoff only. The decision clock is
// sampled in another statement, after the selected row is locked.
const SELECT_CANDIDATE: &str = "SELECT id, event_type, aggregate_type, aggregate_id, \
    schema_version, occurred_at, correlation_id, causation_id, payload, attempt_count, \
    available_at, status, ownership_token, acquired_at, expires_at, retry_cycle, cycle_baseline, cycle_maximum, state_revision FROM relay.outbox_events \
    WHERE status='pending' AND ($2::uuid IS NULL OR id=$2) AND available_at <= $1 AND attempt_count < 2147483647 \
    AND attempt_count - cycle_baseline < cycle_maximum AND state_revision < 9223372036854775807 \
    AND (expires_at IS NULL OR expires_at <= $1) \
    ORDER BY available_at, created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED";

pub(super) async fn begin(
    repository: &PostgresOutboxRepository,
) -> Result<Transaction<'_, Postgres>, PersistenceError> {
    let mut tx = repository.pool.begin().await.map_err(map_driver_error)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await
        .map_err(map_driver_error)?;
    Ok(tx)
}

pub(super) async fn clock(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<DateTime<Utc>, PersistenceError> {
    let row = sqlx::query("SELECT clock_timestamp() AS instant")
        .fetch_one(&mut **tx)
        .await
        .map_err(map_driver_error)?;
    Ok(row
        .try_get::<StoredTimestamp, _>("instant")
        .map_err(invalid_data)?
        .0)
}

impl OutboxAcquirer for PostgresOutboxRepository {
    async fn acquire(
        &self,
        request: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        let mut tx = begin(self).await?;
        let cutoff = clock(&mut tx).await?;
        let row = sqlx::query(SELECT_CANDIDATE)
            .bind(cutoff)
            .bind(request.event_id())
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_driver_error)?;
        let Some(row) = row else {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(None);
        };
        // Restore before mutation, using the exact same checked envelope path as
        // observation. A malformed selected row fails the entire invocation.
        let snapshot = StoredEvent::decode(&row)?.restore()?;
        let now = clock(&mut tx).await?;
        let retry = retry_state(&row, snapshot.attempt_count())?;
        if snapshot.event().id().is_nil() {
            return Err(invalid_data(
                crate::domain::retry::RetryError::InvalidIdentity,
            ));
        }
        if super::discovery::maintenance_eligibility(&row, now, snapshot.attempt_count())?
            != crate::persistence::CandidateEvidence::Valid
        {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(None);
        }

        let status: String = row.try_get("status").map_err(invalid_data)?;
        let expiry = row
            .try_get::<Option<StoredTimestamp>, _>("expires_at")
            .map_err(invalid_data)?
            .map(|time| time.0);
        if status != "pending"
            || snapshot.available_at() > now
            || expiry.is_some_and(|expiry| expiry > now)
            || !retry
                .can_acquire(snapshot.attempt_count())
                .map_err(invalid_data)?
        {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(None);
        }
        let lease =
            DeliveryLease::new(request.token(), now, request.duration()).map_err(|error| {
                PersistenceError::with_source(PersistenceErrorKind::OperationFailed, error)
            })?;
        let attempts = snapshot
            .attempt_count()
            .checked_add(1)
            .ok_or_else(|| PersistenceError::new(PersistenceErrorKind::OperationFailed))?;
        let owned = OwnedOutboxEvent::new(
            PendingOutboxEvent::new(snapshot.event().clone(), attempts, snapshot.available_at()),
            lease,
        )
        .map_err(invalid_data)?
        .with_retry_state(
            RetryState::restore(
                attempts,
                retry.cycle(),
                retry.baseline(),
                retry.maximum(),
                retry.next_revision().map_err(operation_error)?,
            )
            .map_err(invalid_data)?,
        )
        .map_err(invalid_data)?;
        sqlx::query(
            "UPDATE relay.outbox_events SET ownership_token=$2, acquired_at=$3, \
            expires_at=$4, attempt_count=attempt_count+1, state_revision=state_revision+1 WHERE id=$1",
        )
        .bind(owned.key().event_id())
        .bind(lease.token().get())
        .bind(now)
        .bind(lease.expires_at())
        .execute(&mut *tx)
        .await
        .map_err(map_driver_error)?;
        tx.commit().await.map_err(map_commit_error)?;
        Ok(Some(owned))
    }
}

impl PostgresOutboxRepository {
    async fn transition(
        &self,
        owner: OwnedEventKey,
        complete: bool,
    ) -> Result<TransitionOutcome, PersistenceError> {
        let mut tx = begin(self).await?;
        // Lock by identity alone: predicate/clock evaluation must follow any wait.
        let row = sqlx::query(
            "SELECT id, state_revision FROM relay.outbox_events WHERE id=$1 FOR UPDATE",
        )
        .bind(owner.event_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_driver_error)?;
        let Some(_row) = row else {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(TransitionOutcome::OwnershipLost);
        };
        let now = clock(&mut tx).await?;
        let sql = if complete {
            "UPDATE relay.outbox_events SET status='processed', processed_at=$3, \
             ownership_token=NULL, acquired_at=NULL, expires_at=NULL, state_revision=state_revision+1 \
             WHERE id=$1 AND ownership_token=$2 AND status='pending' AND acquired_at <= $3 AND $3 < expires_at"
        } else {
            "UPDATE relay.outbox_events SET ownership_token=NULL, acquired_at=NULL, expires_at=NULL, state_revision=state_revision+1 \
             WHERE id=$1 AND ownership_token=$2 AND status='pending' AND acquired_at <= $3 AND $3 < expires_at"
        };
        let result = sqlx::query(sql)
            .bind(owner.event_id())
            .bind(owner.token().get())
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_driver_error)?;
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(TransitionOutcome::OwnershipLost);
        }
        tx.commit().await.map_err(map_commit_error)?;
        Ok(TransitionOutcome::Applied)
    }
}

impl OutboxCompleter for PostgresOutboxRepository {
    async fn complete(&self, owner: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.transition(owner, true).await
    }
}
impl OutboxReleaser for PostgresOutboxRepository {
    async fn release(&self, owner: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.transition(owner, false).await
    }
}

// An ordinary server rejection is evidence of failed COMMIT. Connection-class,
// completion-unknown and shutdown errors do not establish durable outcome.
fn commit_rejection_is_known(code: Option<&str>) -> bool {
    code.is_some_and(|code| {
        !code.starts_with("08") && !matches!(code, "40003" | "57P01" | "57P02" | "57P03")
    })
}
pub(super) fn map_commit_error(error: sqlx::Error) -> PersistenceError {
    if let sqlx::Error::Database(database) = &error
        && commit_rejection_is_known(database.code().as_deref())
    {
        return map_driver_error(error);
    }
    PersistenceError::with_source(PersistenceErrorKind::CommitUncertain, error)
}

fn operation_error(error: crate::domain::retry::RetryError) -> PersistenceError {
    PersistenceError::with_source(PersistenceErrorKind::OperationFailed, error)
}
pub(super) fn retry_state(
    row: &sqlx::postgres::PgRow,
    lifetime: u32,
) -> Result<RetryState, PersistenceError> {
    RetryState::restore(
        lifetime,
        row.try_get("retry_cycle").map_err(invalid_data)?,
        row.try_get("cycle_baseline").map_err(invalid_data)?,
        row.try_get("cycle_maximum").map_err(invalid_data)?,
        row.try_get("state_revision").map_err(invalid_data)?,
    )
    .map_err(invalid_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn commit_classification_depends_on_acknowledgement() {
        for code in ["40001", "40P01", "23514", "23503", "57014"] {
            assert!(commit_rejection_is_known(Some(code)));
        }
        for code in [
            None,
            Some("08006"),
            Some("08007"),
            Some("40003"),
            Some("57P01"),
        ] {
            assert!(!commit_rejection_is_known(code));
        }
        for source in [
            sqlx::Error::Io(std::io::Error::other("secret")),
            sqlx::Error::Protocol("secret".into()),
            sqlx::Error::WorkerCrashed,
        ] {
            let error = map_commit_error(source);
            assert_eq!(error.kind(), PersistenceErrorKind::CommitUncertain);
            assert!(error.source().unwrap().is::<sqlx::Error>());
            assert!(!format!("{error} {error:?}").contains("secret"));
        }
    }
}
