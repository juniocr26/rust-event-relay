//! Lock order: request advisory transaction lock, then exact event row lock.
//! Hash collisions only serialize unrelated requests; UUID PK enforces identity.
use super::{
    PostgresOutboxRepository, StoredEvent, StoredJson, StoredTimestamp, invalid_data,
    map_driver_error,
    ownership::{begin, clock, map_commit_error, retry_state},
};
use crate::persistence::*;
use chrono::Duration;
use sqlx::Row;
use uuid::Uuid;

impl OutboxReplay for PostgresOutboxRepository {
    async fn replay(&self, request: AuthorizedReplay) -> Result<ReplayOutcome, PersistenceError> {
        if !request.intent.valid() {
            return Err(PersistenceError::new(PersistenceErrorKind::OperationFailed));
        }
        let intent = &request.intent;
        let json = serde_json::to_value(intent).map_err(invalid_data)?;
        let mut tx = begin(self).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 3500))")
            .bind(intent.request_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_driver_error)?;
        let previous = sqlx::query(
            "SELECT actor,scope,intent,receipt FROM relay.replay_requests WHERE request_id=$1",
        )
        .bind(intent.request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_driver_error)?;
        if let Some(previous) = previous {
            let same = previous
                .try_get::<String, _>("actor")
                .map_err(invalid_data)?
                == request.actor
                && previous
                    .try_get::<String, _>("scope")
                    .map_err(invalid_data)?
                    == request.scope
                && previous
                    .try_get::<StoredJson, _>("intent")
                    .map_err(invalid_data)?
                    .0
                    == json;
            let result = if same {
                ReplayOutcome::Reenabled(
                    serde_json::from_value(
                        previous
                            .try_get::<StoredJson, _>("receipt")
                            .map_err(invalid_data)?
                            .0,
                    )
                    .map_err(invalid_data)?,
                )
            } else {
                ReplayOutcome::IntentConflict
            };
            tx.rollback().await.map_err(map_driver_error)?;
            return Ok(result);
        }
        let row = sqlx::query("SELECT *, jsonb_build_object('terminal_at',terminal_at,'terminal_reason',terminal_reason,'diagnostic_code',diagnostic_code,'diagnostic_at',diagnostic_at,'failure_evidence',failure_evidence,'last_error',last_error,'cycle',retry_cycle,'baseline',cycle_baseline,'budget',cycle_maximum,'revision',state_revision,'attempt_count',attempt_count) AS prior_terminal FROM relay.outbox_events WHERE id=$1 FOR UPDATE")
            .bind(intent.event_id).fetch_optional(&mut *tx).await.map_err(map_driver_error)?;
        let result = if let Some(row) = row {
            let status: String = row.try_get("status").map_err(invalid_data)?;
            let revision: i64 = row.try_get("state_revision").map_err(invalid_data)?;
            let cycle: i32 = row.try_get("retry_cycle").map_err(invalid_data)?;
            if status != "dead_letter"
                || revision != intent.expected_revision
                || cycle != intent.expected_cycle
            {
                ReplayOutcome::StateConflict
            } else {
                // Restore original content before mutation; never serialize it back.
                let snapshot = StoredEvent::decode(&row)?.restore()?;
                let lifetime = snapshot.attempt_count();
                let state = retry_state(&row, lifetime)?;
                let terminal = row
                    .try_get::<Option<StoredTimestamp>, _>("terminal_at")
                    .map_err(invalid_data)?;
                let reason: Option<String> =
                    row.try_get("terminal_reason").map_err(invalid_data)?;
                let unsafe_metadata = row
                    .try_get::<Option<Uuid>, _>("ownership_token")
                    .map_err(invalid_data)?
                    .is_some()
                    || row
                        .try_get::<Option<StoredTimestamp>, _>("acquired_at")
                        .map_err(invalid_data)?
                        .is_some()
                    || row
                        .try_get::<Option<StoredTimestamp>, _>("expires_at")
                        .map_err(invalid_data)?
                        .is_some()
                    || row
                        .try_get::<Option<StoredTimestamp>, _>("processed_at")
                        .map_err(invalid_data)?
                        .is_some()
                    || terminal.is_none()
                    || !reason.as_deref().is_some_and(|s| {
                        matches!(
                            s,
                            "legacy_dead_letter"
                                | "serialization_permanent"
                                | "invalid_stored_envelope"
                                | "acquisition_budget_exhausted"
                                | "storage_counter_exhausted"
                        )
                    });
                let diagnostic: Option<String> =
                    row.try_get("diagnostic_code").map_err(invalid_data)?;
                let diagnostic_at = row
                    .try_get::<Option<StoredTimestamp>, _>("diagnostic_at")
                    .map_err(invalid_data)?
                    .map(|t| t.0);
                let evidence = row
                    .try_get::<Option<StoredJson>, _>("failure_evidence")
                    .map_err(invalid_data)?
                    .map(|j| j.0);
                let coherent_diagnostic = match (&diagnostic, diagnostic_at, &evidence) {
                    (None, None, None) => reason.as_deref() == Some("legacy_dead_letter"),
                    (Some(code), Some(at), Some(evidence)) => {
                        valid_evidence(evidence)
                            && matches!(
                                code.as_str(),
                                "unroutable"
                                    | "rejected"
                                    | "serialization_retryable"
                                    | "serialization_permanent"
                                    | "invalid_stored_envelope"
                                    | "acquisition_budget_exhausted"
                                    | "storage_counter_exhausted"
                            )
                            && (reason.as_deref() == Some("legacy_dead_letter")
                                || (Some(code.as_str()) == reason.as_deref()
                                    && terminal.as_ref().is_some_and(|t| t.0 == at)))
                    }
                    _ => false,
                };
                let coherent_reason = (reason.as_deref() != Some("acquisition_budget_exhausted")
                    || state.consumed(lifetime).map_err(invalid_data)? >= state.maximum() as u32)
                    && (reason.as_deref() != Some("storage_counter_exhausted")
                        || lifetime == i32::MAX as u32);
                // Check supported timestamp representation even though created_at is untouched.
                let _: StoredTimestamp = row.try_get("created_at").map_err(invalid_data)?;
                if unsafe_metadata || !coherent_diagnostic || !coherent_reason {
                    ReplayOutcome::Ineligible
                }
                // Reserve capacity for at least one acquisition and its completion.
                else if lifetime == i32::MAX as u32
                    || cycle == i32::MAX
                    || revision > i64::MAX - 3
                {
                    ReplayOutcome::CapacityExhausted
                } else {
                    let now = clock(&mut tx).await?;
                    let available_at = now
                        .checked_add_signed(Duration::milliseconds(intent.delay_ms))
                        .ok_or_else(|| {
                            PersistenceError::new(PersistenceErrorKind::OperationFailed)
                        })?;
                    let receipt = ReplayReceipt {
                        request_id: intent.request_id,
                        event_id: intent.event_id,
                        cycle: cycle + 1,
                        revision: state.next_revision().map_err(invalid_data)?,
                        baseline: lifetime as i32,
                        budget: intent.budget,
                        decided_at: now,
                        available_at,
                    };
                    sqlx::query("UPDATE relay.outbox_events SET status='pending',retry_cycle=$2,cycle_baseline=attempt_count,cycle_maximum=$3,state_revision=$4,available_at=$5,terminal_at=NULL,terminal_reason=NULL,diagnostic_code=NULL,diagnostic_at=NULL,failure_evidence=NULL,last_error=NULL WHERE id=$1")
                        .bind(intent.event_id).bind(receipt.cycle).bind(receipt.budget).bind(receipt.revision).bind(available_at).execute(&mut *tx).await.map_err(map_driver_error)?;
                    sqlx::query("INSERT INTO relay.replay_requests(request_id,actor,scope,event_id,intent,receipt,prior_terminal,recorded_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                        .bind(intent.request_id).bind(&request.actor).bind(&request.scope).bind(intent.event_id).bind(json).bind(serde_json::to_value(&receipt).map_err(invalid_data)?).bind(row.try_get::<StoredJson,_>("prior_terminal").map_err(invalid_data)?.0).bind(now).execute(&mut *tx).await.map_err(map_driver_error)?;
                    tx.commit().await.map_err(map_commit_error)?;
                    return Ok(ReplayOutcome::Reenabled(receipt));
                }
            }
        } else {
            ReplayOutcome::StateConflict
        };
        tx.rollback().await.map_err(map_driver_error)?;
        Ok(result)
    }
    async fn replay_outcome(
        &self,
        actor: &str,
        scope: &str,
        request_id: Uuid,
    ) -> Result<Option<ReplayReceipt>, PersistenceError> {
        let row = sqlx::query("SELECT receipt FROM relay.replay_requests WHERE request_id=$1 AND actor=$2 AND scope=$3")
            .bind(request_id).bind(actor).bind(scope).fetch_optional(&self.pool).await.map_err(map_driver_error)?;
        row.map(|row| {
            serde_json::from_value(
                row.try_get::<StoredJson, _>("receipt")
                    .map_err(invalid_data)?
                    .0,
            )
            .map_err(invalid_data)
        })
        .transpose()
    }
}

fn valid_evidence(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.keys().all(|key| {
        matches!(
            key.as_str(),
            "policy_version" | "phase" | "validator_version" | "evidence_reference"
        )
    }) && value["policy_version"] == "adr008-v1"
        && matches!(
            value["phase"].as_str(),
            Some("before_send" | "broker_response" | "maintenance")
        )
        && object
            .get("validator_version")
            .is_none_or(|v| v == "envelope-v1")
        && object.get("evidence_reference").is_none_or(|v| {
            v.as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| !id.is_nil())
        })
        && value.to_string().len() <= 2048
}
