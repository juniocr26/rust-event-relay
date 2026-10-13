//! Real isolated PostgreSQL retry transitions and failure experiments.
#[path = "support/faults.rs"]
mod faults;
mod support;
use reliable_event_relay::{
    domain::{
        delivery::{LeaseDuration, OwnershipToken},
        retry::*,
    },
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::*,
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use support::{checked, isolated};
use uuid::Uuid;
const UP: &str = include_str!("../migrations/20261012000000_add_durable_retry.up.sql");
const DOWN: &str = include_str!("../migrations/20261012000000_add_durable_retry.down.sql");
const OWN: &str = include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql");
const REJECT: &str = "CREATE FUNCTION relay.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER UPDATE ON relay.outbox_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION relay.reject_commit()";
async fn sql(pool: &PgPool, text: &str) {
    checked(sqlx::raw_sql(text).execute(pool).await, "fixture SQL");
}
async fn setup(pool: &PgPool) {
    sql(pool, OWN).await;
    sql(pool, UP).await;
}
async fn seed(pool: &PgPool, id: u128) {
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,available_at,last_error) VALUES ($1,' event ',' aggregate ',' id ',4294967295,'1999-12-31 23:59:59.123456Z','{\"precise\":18446744073709551617}','2000-01-01','legacy history')").bind(Uuid::from_u128(id)).execute(pool).await,"seed");
}
async fn rows(pool: &PgPool) -> Value {
    checked(
        sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY id),'[]') FROM relay.outbox_events e",
        )
        .fetch_one(pool)
        .await,
        "snapshot",
    )
}
fn request() -> AcquireRequest {
    AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())
}
async fn acquire(repo: &PostgresOutboxRepository) -> OwnedOutboxEvent {
    repo.acquire(request()).await.unwrap().expect("eligible")
}
fn evidence() -> FailureEvidence {
    FailureEvidence::new(
        FailurePhase::BrokerResponse,
        false,
        Some(Uuid::from_u128(42)),
    )
    .unwrap()
}
fn schedule(owned: &OwnedOutboxEvent) -> ScheduleRetry {
    ScheduleRetry {
        owner: RetryOwner::new(owned.key(), owned.retry_state()).unwrap(),
        delay: RetryDelay::from_micros(1_000_000).unwrap(),
        diagnostic: RetryDiagnostic::Rejected,
        evidence: evidence(),
    }
}
fn preserve(owned: &OwnedOutboxEvent) -> PreserveFailure {
    PreserveFailure {
        owner: RetryOwner::new(owned.key(), owned.retry_state()).unwrap(),
        reason: TerminalReason::SerializationPermanent,
        evidence: evidence(),
    }
}
fn exhausted(revision: i64) -> ExhaustedEvent {
    ExhaustedEvent::new(Uuid::from_u128(1), 1, revision).unwrap()
}
fn invariant(mut row: Value) -> Value {
    for field in [
        "status",
        "ownership_token",
        "acquired_at",
        "expires_at",
        "available_at",
        "diagnostic_code",
        "diagnostic_at",
        "failure_evidence",
        "state_revision",
        "terminal_at",
        "terminal_reason",
    ] {
        row.as_object_mut().unwrap().remove(field);
    }
    row
}
async fn wait(tx: &mut Transaction<'_, Postgres>, predicate: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if checked(
                sqlx::query_scalar::<_, bool>(predicate)
                    .fetch_one(&mut **tx)
                    .await,
                "wait predicate",
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("barrier deadline");
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn migration_preserves_all_old_states_leases_and_history_and_isolated_down() {
    isolated(|pool|async move {
  sql(&pool,OWN).await;
  for id in 1..=5 {seed(&pool,id).await;}
  sql(&pool,"UPDATE relay.outbox_events SET attempt_count=12; UPDATE relay.outbox_events SET status='processed',processed_at=created_at WHERE id='00000000-0000-0000-0000-000000000002'; UPDATE relay.outbox_events SET status='dead_letter' WHERE id='00000000-0000-0000-0000-000000000003'; UPDATE relay.outbox_events SET ownership_token=id,acquired_at=clock_timestamp()-interval '1 second',expires_at=clock_timestamp()+interval '1 hour' WHERE id='00000000-0000-0000-0000-000000000004'; UPDATE relay.outbox_events SET ownership_token=id,acquired_at='2000-01-02',expires_at='2000-01-03' WHERE id='00000000-0000-0000-0000-000000000005'").await;
  let before=rows(&pool).await;sql(&pool,UP).await;let after=rows(&pool).await;
  for i in 0..5 {
   for (key,val) in before[i].as_object().unwrap() {assert_eq!(val,&after[i][key],"old field {key}");}
   assert_eq!(after[i]["retry_cycle"],1);assert_eq!(after[i]["cycle_baseline"],0);assert_eq!(after[i]["cycle_maximum"],10);assert_eq!(after[i]["state_revision"],0);
  }
  assert_eq!(after[2]["terminal_reason"],"legacy_dead_letter");assert!(!after[2]["terminal_at"].is_null());
  let migration_time: bool=checked(sqlx::query_scalar("SELECT terminal_at >= created_at AND terminal_at <= clock_timestamp() FROM relay.outbox_events WHERE status='dead_letter'").fetch_one(&pool).await,"migration observation timestamp");assert!(migration_time);
  let repo=PostgresOutboxRepository::new(pool.clone());assert!(repo.acquire(request()).await?.is_none());
  sql(&pool,DOWN).await;assert_eq!(before,rows(&pool).await);sql(&pool,UP).await;
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn schema_rejects_invalid_history_null_evidence_and_terminal_shapes() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;let before=rows(&pool).await;
  for set in ["retry_cycle=0","retry_cycle=-1","cycle_baseline=-1","cycle_baseline=1","cycle_maximum=0","cycle_maximum=-1","state_revision=-1","status='dead_letter'","terminal_at=clock_timestamp()","terminal_reason='serialization_permanent'","diagnostic_code='rejected'","diagnostic_at=clock_timestamp()","failure_evidence='{}'", "diagnostic_code='rejected',diagnostic_at='infinity',failure_evidence='{\"policy_version\":\"adr008-v1\",\"phase\":\"broker_response\"}'", "diagnostic_code='rejected',diagnostic_at=clock_timestamp(),failure_evidence='{\"policy_version\":null,\"phase\":\"broker_response\"}'", "diagnostic_code='rejected',diagnostic_at=clock_timestamp(),failure_evidence='{\"policy_version\":\"adr008-v1\",\"phase\":null}'", "diagnostic_code='rejected',diagnostic_at=clock_timestamp(),failure_evidence='{\"policy_version\":\"adr008-v1\",\"phase\":\"broker_response\",\"validator_version\":null}'", "diagnostic_code='rejected',diagnostic_at=clock_timestamp(),failure_evidence='{\"policy_version\":\"adr008-v1\",\"phase\":\"broker_response\",\"credentials\":\"forbidden\"}'", "status='dead_letter',terminal_at=clock_timestamp(),terminal_reason='serialization_permanent'", "retry_cycle=2147483648","cycle_maximum=2147483648","state_revision=9223372036854775808"] {
   let error=sqlx::query(&format!("UPDATE relay.outbox_events SET {set}")).execute(&pool).await.unwrap_err();
   assert!(matches!(error.as_database_error().and_then(|e|e.code()).as_deref(),Some("23514"|"22003")));
   assert_eq!(before,rows(&pool).await);
  }
  // Bound applies to serialized UTF-8 JSON, independent of caller models.
  let big=serde_json::json!({"policy_version":"adr008-v1","phase":"broker_response","evidence_reference":"x".repeat(2048)});
  assert!(sqlx::query("UPDATE relay.outbox_events SET diagnostic_code='rejected',diagnostic_at=clock_timestamp(),failure_evidence=$1").bind(big).execute(&pool).await.is_err());
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn schedule_is_atomic_preserves_history_and_blocks_early_acquisition() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;let repo=PostgresOutboxRepository::new(pool.clone());let owned=acquire(&repo).await;
  let before=rows(&pool).await;let req=schedule(&owned);
  let RetryTransition::Applied(result)=repo.schedule_retry(req).await? else {panic!("schedule");};
  let after=rows(&pool).await;assert_eq!(invariant(before[0].clone()),invariant(after[0].clone()));
  assert_eq!(after[0]["status"],"pending");assert_eq!(after[0]["state_revision"],2);assert_eq!(result.revision,2);
  assert_eq!(after[0]["failure_evidence"],evidence().to_json());assert_eq!(after[0]["diagnostic_code"],"rejected");
  let delta:i64=checked(sqlx::query_scalar("SELECT (extract(epoch FROM (available_at-diagnostic_at))*1000000)::bigint FROM relay.outbox_events").fetch_one(&pool).await,"delay");assert_eq!(delta,1_000_000);
  for field in ["ownership_token","acquired_at","expires_at"] {assert!(after[0][field].is_null());}
  assert!(repo.acquire(request()).await?.is_none());assert_eq!(repo.schedule_retry(req).await?,RetryTransition::Conflict);assert_eq!(after,rows(&pool).await);
  sql(&pool,"UPDATE relay.outbox_events SET available_at='2000-01-01'").await;
  let next=acquire(&repo).await;assert_eq!(next.snapshot().event(),owned.snapshot().event());assert_eq!(next.snapshot().attempt_count(),2);assert_eq!(next.retry_state().revision(),3);
  assert_eq!(repo.release(next.key()).await?,TransitionOutcome::Applied);
  let released=rows(&pool).await;assert_eq!(released[0]["failure_evidence"],after[0]["failure_evidence"]);assert_eq!(released[0]["last_error"],before[0]["last_error"]);assert_eq!(released[0]["state_revision"],4);
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn owned_transitions_reject_identity_cycle_revision_and_expired_authority() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let owned = acquire(&repo).await;
        for owner in [
            RetryOwner::new(
                OwnedEventKey::new(Uuid::from_u128(99), owned.key().token()),
                owned.retry_state(),
            )
            .unwrap(),
            RetryOwner::new(
                OwnedEventKey::new(owned.key().event_id(), OwnershipToken::fresh()),
                owned.retry_state(),
            )
            .unwrap(),
            RetryOwner::new(owned.key(), RetryState::restore(1, 2, 0, 10, 1).unwrap()).unwrap(),
            RetryOwner::new(owned.key(), RetryState::restore(1, 1, 0, 10, 0).unwrap()).unwrap(),
        ] {
            let before = rows(&pool).await;
            let mut req = schedule(&owned);
            req.owner = owner;
            assert_eq!(repo.schedule_retry(req).await?, RetryTransition::Conflict);
            let mut req = preserve(&owned);
            req.owner = owner;
            assert_eq!(repo.preserve_failure(req).await?, RetryTransition::Conflict);
            assert_eq!(before, rows(&pool).await);
        }
        for set in [
            "acquired_at='2000-01-02',expires_at='2000-01-03'",
            "acquired_at='9998-01-01',expires_at='9999-01-01'",
        ] {
            sql(&pool, &format!("UPDATE relay.outbox_events SET {set}")).await;
            let before = rows(&pool).await;
            assert_eq!(
                repo.schedule_retry(schedule(&owned)).await?,
                RetryTransition::Conflict
            );
            assert_eq!(
                repo.preserve_failure(preserve(&owned)).await?,
                RetryTransition::Conflict
            );
            assert_eq!(before, rows(&pool).await);
        }
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn terminalization_preserves_event_and_is_not_success_or_repeat_success() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let owned = acquire(&repo).await;
        let before = rows(&pool).await;
        let req = preserve(&owned);
        let RetryTransition::Applied(result) = repo.preserve_failure(req).await? else {
            panic!("preserve");
        };
        let after = rows(&pool).await;
        assert_eq!(invariant(before[0].clone()), invariant(after[0].clone()));
        assert_eq!(result.reason, TerminalReason::SerializationPermanent);
        assert_eq!(result.revision, 2);
        assert_eq!(after[0]["status"], "dead_letter");
        assert!(after[0]["processed_at"].is_null());
        assert_eq!(after[0]["terminal_at"], after[0]["diagnostic_at"]);
        assert_eq!(after[0]["failure_evidence"], evidence().to_json());
        assert!(repo.acquire(request()).await?.is_none());
        assert_eq!(repo.preserve_failure(req).await?, RetryTransition::Conflict);
        assert_eq!(
            repo.complete(owned.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(after, rows(&pool).await);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn exact_budget_boundary_final_completion_and_no_scheduling_bypass() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        for n in 1..=10 {
            let owned = acquire(&repo).await;
            assert_eq!(owned.snapshot().attempt_count(), n);
            if n < 10 {
                assert_eq!(repo.release(owned.key()).await?, TransitionOutcome::Applied);
            } else {
                let before = rows(&pool).await;
                assert_eq!(
                    repo.schedule_retry(schedule(&owned)).await?,
                    RetryTransition::Conflict
                );
                assert_eq!(
                    repo.preserve_exhausted(exhausted(19)).await?,
                    RetryTransition::Conflict
                );
                assert_eq!(before, rows(&pool).await);
                assert_eq!(
                    repo.complete(owned.key()).await?,
                    TransitionOutcome::Applied
                );
            }
        }
        assert!(repo.acquire(request()).await?.is_none());
        let after = rows(&pool).await;
        assert_eq!(after[0]["cycle_baseline"], 0);
        assert_eq!(after[0]["attempt_count"], 10);
        assert_eq!(after[0]["state_revision"], 20);
        assert_eq!(after[0]["status"], "processed");
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn concurrent_acquirers_cannot_overspend_and_final_failure_can_terminalize() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        sql(&pool, "UPDATE relay.outbox_events SET attempt_count=9").await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let (a, b) = tokio::join!(repo.acquire(request()), repo.acquire(request()));
        let owned = match (a?, b?) {
            (Some(x), None) | (None, Some(x)) => x,
            _ => panic!("exactly one final owner"),
        };
        let mut req = preserve(&owned);
        req.reason = TerminalReason::AcquisitionBudgetExhausted;
        assert!(matches!(
            repo.preserve_failure(req).await?,
            RetryTransition::Applied(_)
        ));
        assert!(repo.acquire(request()).await?.is_none());
        assert_eq!(rows(&pool).await[0]["attempt_count"], 10);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn maintenance_checks_due_state_revision_live_lease_and_storage_exhaustion() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;let repo=PostgresOutboxRepository::new(pool.clone());
  assert_eq!(repo.preserve_exhausted(exhausted(0)).await?,RetryTransition::Conflict);
  sql(&pool,"UPDATE relay.outbox_events SET attempt_count=10,available_at='9999-01-01'").await;
  assert_eq!(repo.preserve_exhausted(exhausted(0)).await?,RetryTransition::Conflict);
  sql(&pool,"UPDATE relay.outbox_events SET available_at='2000-01-01',ownership_token=id,acquired_at=clock_timestamp(),expires_at=clock_timestamp()+interval '1 hour'").await;
  assert_eq!(repo.preserve_exhausted(exhausted(0)).await?,RetryTransition::Conflict);
  sql(&pool,"UPDATE relay.outbox_events SET acquired_at='2000-01-02',expires_at='2000-01-03'").await;
  for req in [exhausted(1),ExhaustedEvent::new(Uuid::from_u128(1),2,0).unwrap(),ExhaustedEvent::new(Uuid::from_u128(99),1,0).unwrap()] {assert_eq!(repo.preserve_exhausted(req).await?,RetryTransition::Conflict);}
  let before=rows(&pool).await;let RetryTransition::Applied(result)=repo.preserve_exhausted(exhausted(0)).await? else {panic!("expired exhausted");};assert_eq!(result.reason,TerminalReason::AcquisitionBudgetExhausted);
  let after=rows(&pool).await;assert_eq!(invariant(before[0].clone()),invariant(after[0].clone()));assert!(after[0]["ownership_token"].is_null());assert!(after[0]["processed_at"].is_null());assert_eq!(repo.preserve_exhausted(exhausted(0)).await?,RetryTransition::Conflict);
  sql(&pool,"DELETE FROM relay.outbox_events").await;seed(&pool,1).await;sql(&pool,"UPDATE relay.outbox_events SET attempt_count=2147483647,cycle_baseline=2147483647").await;
  assert!(repo.acquire(request()).await?.is_none());let RetryTransition::Applied(result)=repo.preserve_exhausted(exhausted(0)).await? else {panic!("storage exhausted");};assert_eq!(result.reason,TerminalReason::StorageCounterExhausted);assert_eq!(rows(&pool).await[0]["attempt_count"],i32::MAX);
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn maintenance_competes_safely_and_never_invents_envelope_validity() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        sql(
            &pool,
            "UPDATE relay.outbox_events SET attempt_count=10,event_type=' '",
        )
        .await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let (a, b) = tokio::join!(
            repo.preserve_exhausted(exhausted(0)),
            repo.preserve_exhausted(exhausted(0))
        );
        assert!(matches!(
            (a?, b?),
            (RetryTransition::Applied(_), RetryTransition::Conflict)
                | (RetryTransition::Conflict, RetryTransition::Applied(_))
        ));
        assert_eq!(rows(&pool).await[0]["event_type"], " ");
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn scheduling_samples_time_after_lock_and_expired_waiters_are_rejected() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;let repo=PostgresOutboxRepository::new(pool.clone());
  for expire in [false,true] {
   let owned=repo.acquire(AcquireRequest::new(LeaseDuration::from_micros(if expire {600_000} else {30_000_000}).unwrap())).await?.unwrap();
   let req=schedule(&owned);let mut blocker=checked(pool.begin().await,"blocker");checked(sqlx::query("SELECT id FROM relay.outbox_events FOR UPDATE").fetch_one(&mut *blocker).await,"lock");
   let r=repo.clone();let task=tokio::spawn(async move {r.schedule_retry(req).await});
   wait(&mut blocker,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").await;
   if expire {wait(&mut blocker,"SELECT clock_timestamp()>=expires_at FROM relay.outbox_events").await;}
   else {checked(sqlx::query("SELECT pg_sleep(0.05)").execute(&mut *blocker).await,"controlled lock hold");}
   let unlock:chrono::DateTime<chrono::Utc>=checked(sqlx::query_scalar("SELECT clock_timestamp()").fetch_one(&mut *blocker).await,"unlock time");
   checked(blocker.commit().await,"unlock");
   let result=task.await.unwrap()?;
   if expire {assert_eq!(result,RetryTransition::Conflict);} else {
    let RetryTransition::Applied(result)=result else {panic!("schedule after wait");};assert!(result.available_at >= unlock+chrono::Duration::seconds(1));
    sql(&pool,"UPDATE relay.outbox_events SET available_at='2000-01-01'").await;
   }
  }
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn terminal_waiters_lose_expired_authority_and_maintenance_skips_completion_lock() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;sql(&pool,"UPDATE relay.outbox_events SET attempt_count=9").await;let repo=PostgresOutboxRepository::new(pool.clone());
  let owned=repo.acquire(AcquireRequest::new(LeaseDuration::from_micros(600_000).unwrap())).await?.unwrap();
  let mut blocker=checked(pool.begin().await,"blocker");checked(sqlx::query("SELECT id FROM relay.outbox_events FOR UPDATE").fetch_one(&mut *blocker).await,"lock");
  let req=preserve(&owned);let r=repo.clone();let task=tokio::spawn(async move {r.preserve_failure(req).await});
  wait(&mut blocker,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").await;
  wait(&mut blocker,"SELECT clock_timestamp()>=expires_at FROM relay.outbox_events").await;checked(blocker.commit().await,"unlock");assert_eq!(task.await.unwrap()?,RetryTransition::Conflict);
  // Restore only test-owned live lease; production never extends it this way.
  sql(&pool,"UPDATE relay.outbox_events SET acquired_at=clock_timestamp(),expires_at=clock_timestamp()+interval '1 hour'").await;
  let (completion,maintenance)=tokio::join!(repo.complete(owned.key()),repo.preserve_exhausted(exhausted(1)));
  assert_eq!(completion?,TransitionOutcome::Applied);assert_eq!(maintenance?,RetryTransition::Conflict);assert_eq!(rows(&pool).await[0]["status"],"processed");
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn revision_and_counter_storage_boundaries_fail_without_partial_changes() {
    isolated(|pool|async move {
  setup(&pool).await;seed(&pool,1).await;sql(&pool,"UPDATE relay.outbox_events SET attempt_count=2147483646,cycle_baseline=2147483646").await;
  let repo=PostgresOutboxRepository::new(pool.clone());let owned=acquire(&repo).await;assert_eq!(owned.snapshot().attempt_count(),i32::MAX as u32);assert!(repo.acquire(request()).await?.is_none());assert_eq!(repo.complete(owned.key()).await?,TransitionOutcome::Applied);
  sql(&pool,"DELETE FROM relay.outbox_events").await;seed(&pool,1).await;let owned=acquire(&repo).await;
  sql(&pool,"UPDATE relay.outbox_events SET state_revision=9223372036854775807").await;
  let owner=RetryOwner::new(owned.key(),RetryState::restore(1,1,0,10,i64::MAX).unwrap()).unwrap();
  let mut req=schedule(&owned);req.owner=owner;let before=rows(&pool).await;
  assert_eq!(repo.schedule_retry(req).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
  let mut req=preserve(&owned);req.owner=owner;assert_eq!(repo.preserve_failure(req).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
  assert_eq!(repo.complete(owned.key()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);assert_eq!(repo.release(owned.key()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);assert_eq!(before,rows(&pool).await);
  sql(&pool,"UPDATE relay.outbox_events SET ownership_token=NULL,acquired_at=NULL,expires_at=NULL,attempt_count=10").await;let before=rows(&pool).await;
  assert_eq!(repo.preserve_exhausted(exhausted(i64::MAX)).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);assert!(repo.acquire(request()).await?.is_none());assert_eq!(before,rows(&pool).await);
  Ok(())
 }).await;
}
async fn mutation(
    repo: &PostgresOutboxRepository,
    owned: &OwnedOutboxEvent,
    op: usize,
) -> Result<bool, PersistenceError> {
    match op {
        0 => Ok(matches!(
            repo.schedule_retry(schedule(owned)).await?,
            RetryTransition::Applied(_)
        )),
        1 => Ok(matches!(
            repo.preserve_failure(preserve(owned)).await?,
            RetryTransition::Applied(_)
        )),
        _ => Ok(matches!(
            repo.preserve_exhausted(exhausted(1)).await?,
            RetryTransition::Applied(_)
        )),
    }
}
async fn prepare(pool: &PgPool, op: usize) -> OwnedOutboxEvent {
    sql(pool, "DELETE FROM relay.outbox_events").await;
    seed(pool, 1).await;
    let repo = PostgresOutboxRepository::new(pool.clone());
    let owned = acquire(&repo).await;
    if op == 2 {
        sql(pool,"UPDATE relay.outbox_events SET attempt_count=10,ownership_token=NULL,acquired_at=NULL,expires_at=NULL").await;
    }
    owned
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn statement_and_commit_rejections_roll_back_every_new_mutation() {
    isolated(|pool|async move {
  setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());
  for deferred in [false,true] {for op in 0..3 {
   let owned=prepare(&pool,op).await;
   if deferred {sql(&pool,REJECT).await;} else {sql(&pool,"CREATE FUNCTION relay.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_commit BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.reject_commit()").await;}
   let before=rows(&pool).await;
   assert_eq!(mutation(&repo,&owned,op).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);assert_eq!(before,rows(&pool).await);
   sql(&pool,"DROP TRIGGER reject_commit ON relay.outbox_events; DROP FUNCTION relay.reject_commit()").await;
  }}
  Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn lost_commit_ack_new_transitions_are_observed_independently_never_blindly_retried() {
    isolated(|pool| async move {
        setup(&pool).await;
        let direct = PostgresOutboxRepository::new(pool.clone());
        for op in 0..3 {
            let owned = prepare(&pool, op).await;
            let (proxy_pool, proxy) = faults::lost_commit_ack_pool(&pool).await;
            let repo = PostgresOutboxRepository::new(proxy_pool.clone());
            let error = tokio::time::timeout(Duration::from_secs(5), mutation(&repo, &owned, op))
                .await
                .unwrap()
                .unwrap_err();
            assert_eq!(error.kind(), PersistenceErrorKind::CommitUncertain);
            assert!(proxy.await.unwrap());
            proxy_pool.close().await;
            let durable = rows(&pool).await;
            assert_eq!(durable[0]["state_revision"], 2);
            assert_eq!(
                durable[0]["status"],
                if op == 0 { "pending" } else { "dead_letter" }
            );
            assert!(durable[0]["ownership_token"].is_null());
            assert!(!durable[0]["failure_evidence"].is_null());
            assert!(!mutation(&direct, &owned, op).await?);
            assert_eq!(durable, rows(&pool).await);
        }
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn cancellation_of_owned_lock_waits_leaves_no_partial_changes_and_maintenance_skips_lock() {
    isolated(|pool|async move {
  setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());
  for op in 0..2 {
   let owned=prepare(&pool,op).await;let before=rows(&pool).await;let mut blocker=checked(pool.begin().await,"blocker");checked(sqlx::query("SELECT id FROM relay.outbox_events FOR UPDATE").fetch_one(&mut *blocker).await,"lock");
   let r=repo.clone();let task=tokio::spawn(async move {mutation(&r,&owned,op).await});wait(&mut blocker,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock')").await;
   task.abort();assert!(task.await.unwrap_err().is_cancelled());checked(blocker.rollback().await,"unlock");assert_eq!(before,rows(&pool).await);
  }
  // Independently locked raw row is skipped, not overwritten.
  let _=prepare(&pool,2).await;let mut blocker=checked(pool.begin().await,"blocker");checked(sqlx::query("SELECT id FROM relay.outbox_events FOR UPDATE").fetch_one(&mut *blocker).await,"lock");assert_eq!(repo.preserve_exhausted(exhausted(1)).await?,RetryTransition::Conflict);checked(blocker.rollback().await,"unlock");
  Ok(())
 }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn cancellation_after_server_commit_suppresses_results_but_not_durable_mutations() {
    isolated(|pool| async move {
        setup(&pool).await;
        for op in 0..3 {
            let owned = prepare(&pool, op).await;
            let (proxy_pool, proxy, observed) = faults::withheld_commit_ack_pool(&pool).await;
            let repo = PostgresOutboxRepository::new(proxy_pool.clone());
            let task = tokio::spawn(async move { mutation(&repo, &owned, op).await });
            tokio::time::timeout(Duration::from_secs(5), observed)
                .await
                .unwrap()
                .unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            proxy.abort();
            let _ = proxy.await;
            tokio::time::timeout(Duration::from_secs(5), proxy_pool.close())
                .await
                .unwrap();
            let durable = rows(&pool).await;
            assert_eq!(durable[0]["state_revision"], 2);
            assert!(durable[0]["ownership_token"].is_null());
            assert_eq!(
                durable[0]["status"],
                if op == 0 { "pending" } else { "dead_letter" }
            );
        }
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn persisted_noninitial_cycle_allowance_and_terminal_reason_evidence_are_enforced() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        sql(&pool, "UPDATE relay.outbox_events SET retry_cycle=2, cycle_baseline=5, attempt_count=5, cycle_maximum=2, state_revision=20").await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let owned = acquire(&repo).await;
        assert_eq!(owned.retry_state(), RetryState::restore(6, 2, 5, 2, 21).unwrap());
        let before = rows(&pool).await;
        for reason in [TerminalReason::AcquisitionBudgetExhausted, TerminalReason::StorageCounterExhausted] {
            let mut req = preserve(&owned);
            req.reason = reason;
            assert_eq!(repo.preserve_failure(req).await?, RetryTransition::Conflict);
            assert_eq!(before, rows(&pool).await);
        }
        assert!(matches!(repo.schedule_retry(schedule(&owned)).await?, RetryTransition::Applied(_)));
        sql(&pool, "UPDATE relay.outbox_events SET available_at='2000-01-01'").await;
        let final_owner = acquire(&repo).await;
        assert_eq!(final_owner.snapshot().attempt_count(), 7);
        assert_eq!(final_owner.retry_state().revision(), 23);
        assert_eq!(repo.schedule_retry(schedule(&final_owner)).await?, RetryTransition::Conflict);
        assert_eq!(repo.release(final_owner.key()).await?, TransitionOutcome::Applied);
        assert!(repo.acquire(request()).await?.is_none());
        let req = ExhaustedEvent::new(Uuid::from_u128(1), 2, 24).unwrap();
        assert!(matches!(repo.preserve_exhausted(req).await?, RetryTransition::Applied(_)));
        let after = rows(&pool).await;
        assert_eq!(after[0]["retry_cycle"], 2);
        assert_eq!(after[0]["cycle_baseline"], 5);
        assert_eq!(after[0]["cycle_maximum"], 2);
        assert_eq!(after[0]["attempt_count"], 7);
        assert_eq!(after[0]["state_revision"], 25);
        Ok(())
    }).await;
}

/// Injected definitive rejection; these cases establish PostgreSQL acknowledgement
/// loss through the real use case, not a real RabbitMQ NACK.
struct RejectedPublisher;
impl reliable_event_relay::application::publisher::EventPublisher for RejectedPublisher {
    async fn publish(
        &self,
        _: &reliable_event_relay::domain::event::EventEnvelope,
    ) -> Result<(), reliable_event_relay::application::publisher::PublishError> {
        use reliable_event_relay::application::publisher::*;
        Err(PublishError::new(PublishErrorKind::Rejected))
    }
}
struct TransitionConnection {
    direct: PostgresOutboxRepository,
    transition: PostgresOutboxRepository,
}
impl OutboxAcquirer for TransitionConnection {
    async fn acquire(
        &self,
        r: AcquireRequest,
    ) -> Result<Option<OwnedOutboxEvent>, PersistenceError> {
        self.direct.acquire(r).await
    }
}
impl OutboxCompleter for TransitionConnection {
    async fn complete(&self, key: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.direct.complete(key).await
    }
}
impl OutboxRetryScheduler for TransitionConnection {
    async fn schedule_retry(
        &self,
        r: ScheduleRetry,
    ) -> Result<RetryTransition<ScheduledRetry>, PersistenceError> {
        self.transition.schedule_retry(r).await
    }
}
impl OutboxFailurePreserver for TransitionConnection {
    async fn preserve_failure(
        &self,
        r: PreserveFailure,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        self.transition.preserve_failure(r).await
    }
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn delivery_reports_lost_schedule_and_terminal_ack_without_repeating_or_erasing_primary() {
    use reliable_event_relay::application::{
        delivery::*, publisher::*, retry_policy::selected_delay,
    };
    isolated(|pool| async move {
        setup(&pool).await;
        for terminal in [false, true] {
            sql(&pool, "DELETE FROM relay.outbox_events").await;
            seed(&pool, 1).await;
            if terminal {
                sql(&pool, "UPDATE relay.outbox_events SET attempt_count=9").await;
            }
            let (proxy_pool, proxy) = faults::lost_commit_ack_pool(&pool).await;
            let repo = TransitionConnection {
                direct: PostgresOutboxRepository::new(pool.clone()),
                transition: PostgresOutboxRepository::new(proxy_pool.clone()),
            };
            let out = tokio::time::timeout(
                Duration::from_secs(5),
                deliver_once_with_delay(&repo, &RejectedPublisher, request(), |n| {
                    selected_delay(n, 0)
                }),
            )
            .await
            .unwrap();
            let DeliveryOutcome::PublicationFailed {
                error, handling, ..
            } = out
            else {
                panic!("publication failure")
            };
            assert_eq!(error.kind(), PublishErrorKind::Rejected);
            let error = match handling {
                FailureHandling::Scheduled(Err(error)) if !terminal => error,
                FailureHandling::Terminal(Err(error)) if terminal => error,
                other => panic!("expected lost acknowledgement: {other:?}"),
            };
            assert_eq!(error.kind(), PersistenceErrorKind::CommitUncertain);
            assert!(proxy.await.unwrap());
            proxy_pool.close().await;
            let durable = rows(&pool).await;
            assert_eq!(durable[0]["state_revision"], 2); // acquisition + exactly one transition
            assert_eq!(durable[0]["attempt_count"], if terminal { 10 } else { 1 });
            assert_eq!(
                durable[0]["status"],
                if terminal { "dead_letter" } else { "pending" }
            );
            assert!(durable[0]["processed_at"].is_null());
            assert!(durable[0]["ownership_token"].is_null());
            assert!(repo.direct.acquire(request()).await?.is_none());
        }
        Ok(())
    })
    .await;
}

impl OutboxReleaser for TransitionConnection {
    async fn release(&self, key: OwnedEventKey) -> Result<TransitionOutcome, PersistenceError> {
        self.direct.release(key).await
    }
}
