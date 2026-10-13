//! Raw discovery and fresh maintenance authority. No driver data escapes.
use super::{
    PostgresOutboxRepository, StoredEvent, StoredTimestamp, invalid_data, map_driver_error,
    ownership::{begin, clock, map_commit_error, retry_state},
    retry::terminal,
};
use crate::{
    domain::{
        event::EventError,
        retry::{FailureEvidence, FailurePhase, TerminalReason},
    },
    persistence::*,
};
use chrono::{DateTime, Utc};
use sqlx::{Row, postgres::PgRow};
use std::error::Error;
use uuid::Uuid;

// The primary-key traversal has no OFFSET or repeatedly selected poisoned prefix.
const COLUMNS: &str = "id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,correlation_id,causation_id,payload,attempt_count,available_at,status,ownership_token,acquired_at,expires_at,retry_cycle,cycle_baseline,cycle_maximum,state_revision";
fn timestamp(row: &PgRow, column: &str) -> Result<Option<DateTime<Utc>>, PersistenceError> {
    Ok(row
        .try_get::<Option<StoredTimestamp>, _>(column)
        .map_err(invalid_data)?
        .map(|t| t.0))
}
pub(super) fn maintenance_eligibility(
    row: &PgRow,
    now: DateTime<Utc>,
    count: u32,
) -> Result<CandidateEvidence, PersistenceError> {
    let due = timestamp(row, "available_at")?
        .ok_or_else(|| PersistenceError::new(PersistenceErrorKind::InvalidStoredData))?;
    let acquired = timestamp(row, "acquired_at")?;
    let expires = timestamp(row, "expires_at")?;
    let token: Option<Uuid> = row.try_get("ownership_token").map_err(invalid_data)?;
    let valid_owner = match (token, acquired, expires) {
        (None, None, None) => true,
        (Some(token), Some(start), Some(end)) => {
            !token.is_nil() && count > 0 && due <= start && start < end && start <= now
        }
        _ => false,
    };
    if !valid_owner {
        return Ok(CandidateEvidence::RepairRequired);
    }
    if expires.is_some_and(|t| t > now) {
        return Ok(CandidateEvidence::LiveOwner);
    }
    if due > now {
        return Ok(CandidateEvidence::NotDue);
    }
    Ok(CandidateEvidence::Valid)
}
fn classify(
    row: &PgRow,
    now: DateTime<Utc>,
) -> Result<(ExhaustedEvent, CandidateEvidence), PersistenceError> {
    let id: Uuid = row.try_get("id").map_err(invalid_data)?;
    let count = u32::try_from(
        row.try_get::<i32, _>("attempt_count")
            .map_err(invalid_data)?,
    )
    .map_err(invalid_data)?;
    let state = retry_state(row, count)?;
    let expected =
        ExhaustedEvent::new(id, state.cycle(), state.revision()).map_err(invalid_data)?;
    state.next_revision().map_err(invalid_data)?;
    let eligibility = maintenance_eligibility(row, now, count)?;
    if eligibility != CandidateEvidence::Valid {
        return Ok((expected, eligibility));
    }
    // Decoding is not domain validation. Unsupported timestamp/JSONB/driver
    // representations never become permanent-invalid attestation.
    let stored = match StoredEvent::decode(row) {
        Ok(stored) => stored,
        Err(_) => return Ok((expected, CandidateEvidence::UnsupportedRepresentation)),
    };
    let envelope = match stored.restore() {
        Ok(_) => CandidateEvidence::Valid,
        Err(error) => match error
            .source()
            .and_then(|source| source.downcast_ref::<EventError>())
        {
            Some(error) => CandidateEvidence::InvalidEnvelope(error.clone()),
            None => CandidateEvidence::UnsupportedRepresentation,
        },
    };
    // Budget classification requires a restorable envelope. Never use budget
    // exhaustion to overwrite unsupported evidence or unsafe authority.
    let evidence = if envelope == CandidateEvidence::Valid {
        if count == i32::MAX as u32 {
            CandidateEvidence::Exhausted(TerminalReason::StorageCounterExhausted)
        } else if !state.can_acquire(count).map_err(invalid_data)? {
            CandidateEvidence::Exhausted(TerminalReason::AcquisitionBudgetExhausted)
        } else {
            envelope
        }
    } else {
        envelope
    };
    Ok((expected, evidence))
}
impl OutboxCandidateDiscovery for PostgresOutboxRepository {
    async fn discover_candidate(
        &self,
        cursor: DiscoveryCursor,
    ) -> Result<Option<DiscoveryObservation>, PersistenceError> {
        let sql = format!(
            "WITH boundary AS (SELECT coalesce($2::uuid,(SELECT id FROM relay.outbox_events ORDER BY id DESC LIMIT 1)) AS through) SELECT {COLUMNS},boundary.through,clock_timestamp() AS observed_at FROM relay.outbox_events CROSS JOIN boundary WHERE status='pending' AND ($1::uuid IS NULL OR id > $1) AND id <= boundary.through ORDER BY id LIMIT 1"
        );
        let Some(row) = sqlx::query(&sql)
            .bind(cursor.after)
            .bind(cursor.through)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_driver_error)?
        else {
            return Ok(None);
        };
        let id = row.try_get("id").map_err(invalid_data)?;
        let now = row
            .try_get::<StoredTimestamp, _>("observed_at")
            .map_err(invalid_data)?
            .0;
        let (evidence, candidate) = match classify(&row, now) {
            Ok((expected, evidence)) => {
                let candidate = evidence.terminal_reason().map(|_| MaintenanceCandidate {
                    expected,
                    evidence: evidence.clone(),
                });
                (evidence, candidate)
            }
            Err(_) => (CandidateEvidence::RepairRequired, None),
        };
        Ok(Some(DiscoveryObservation {
            cursor: DiscoveryCursor {
                after: Some(id),
                through: Some(row.try_get("through").map_err(invalid_data)?),
            },
            evidence,
            candidate,
        }))
    }
}
impl OutboxCandidateIsolator for PostgresOutboxRepository {
    async fn isolate_candidate(
        &self,
        candidate: MaintenanceCandidate,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        let mut tx = begin(self).await?;
        let sql =
            format!("SELECT {COLUMNS} FROM relay.outbox_events WHERE id=$1 FOR UPDATE SKIP LOCKED");
        let row = sqlx::query(&sql)
            .bind(candidate.expected.id())
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_driver_error)?;
        let now = clock(&mut tx).await?;
        let decision = row.as_ref().and_then(|row| {
            let status: String = row.try_get("status").ok()?;
            if status != "pending" {
                return None;
            }
            let (expected, evidence) = classify(row, now).ok()?;
            if expected.cycle() != candidate.expected.cycle()
                || expected.revision() != candidate.expected.revision()
                || evidence != candidate.evidence
            {
                return None;
            }
            evidence.terminal_reason()
        });
        let Some(reason) = decision else {
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(RetryTransition::Conflict);
        };
        let revision = candidate
            .expected
            .revision()
            .checked_add(1)
            .ok_or_else(|| PersistenceError::new(PersistenceErrorKind::OperationFailed))?;
        let evidence = FailureEvidence::new(
            FailurePhase::Maintenance,
            reason == TerminalReason::InvalidStoredEnvelope,
            Some(candidate.expected.id()),
        )
        .map_err(invalid_data)?;
        terminal(
            &mut tx,
            candidate.expected.id(),
            now,
            reason,
            evidence,
            revision,
        )
        .await?;
        tx.commit().await.map_err(map_commit_error)?;
        Ok(RetryTransition::Applied(PreservedFailure {
            terminal_at: now,
            reason,
            revision,
        }))
    }
}
