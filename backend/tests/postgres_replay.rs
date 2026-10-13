#[path = "support/faults.rs"]
mod faults;
mod support;
use reliable_event_relay::{
    application::replay::{self, ReplayPrincipal},
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::*,
};
use sqlx::{PgPool, Row};
use support::{checked, isolated};
use uuid::Uuid;
async fn setup(pool: &PgPool) {
    for migration in [
        include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
        include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),
        include_str!("../migrations/20261013000000_add_audited_replay.up.sql"),
    ] {
        checked(
            sqlx::raw_sql(migration).execute(pool).await,
            "replay migration",
        );
    }
}
async fn seed(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    checked(sqlx::query("INSERT INTO relay.outbox_events(id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,status,attempt_count,state_revision,terminal_at,terminal_reason,last_error) VALUES($1,'test','test','1',1,clock_timestamp(),'{\"exact\":123456789012345678901234567890}', 'dead_letter',7,4,clock_timestamp(),'legacy_dead_letter','restricted prior evidence')").bind(id).execute(pool).await,"seed terminal");
    id
}
fn intent(id: Uuid) -> ReplayIntent {
    ReplayIntent {
        request_id: Uuid::now_v7(),
        event_id: id,
        expected_revision: 4,
        expected_cycle: 1,
        reason: "route repaired".into(),
        budget: 10,
        delay_ms: 0,
        ticket: Some(Uuid::now_v7()),
    }
}
fn principal() -> ReplayPrincipal {
    ReplayPrincipal::authenticated("operator".into(), true, 10)
}
async fn apply(repo: &PostgresOutboxRepository, i: ReplayIntent) -> ReplayOutcome {
    replay::replay(repo, &principal(), i).await.unwrap()
}
async fn snapshot(pool: &PgPool, id: Uuid) -> serde_json::Value {
    checked(sqlx::query_scalar("SELECT to_jsonb(e) - ARRAY['status','retry_cycle','cycle_baseline','cycle_maximum','state_revision','available_at','terminal_at','terminal_reason','diagnostic_code','diagnostic_at','failure_evidence','last_error'] FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(pool).await,"original snapshot")
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn atomic_preservation_idempotency_authorization_and_fresh_acquisition() {
    isolated(|pool| async move {
        setup(&pool).await;
        let id = seed(&pool).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let before = snapshot(&pool, id).await;
        let i = intent(id);
        let denied = ReplayPrincipal::authenticated("operator".into(), false, 10);
        assert!(replay::replay(&repo, &denied, i.clone()).await.is_err());
        assert!(replay::outcome(&repo, &denied, i.request_id).await.is_err());
        let ReplayOutcome::Reenabled(receipt) = apply(&repo, i.clone()).await else {
            panic!("replay refused")
        };
        assert_eq!(
            (
                receipt.cycle,
                receipt.revision,
                receipt.baseline,
                receipt.budget
            ),
            (2, 5, 7, 10)
        );
        assert_eq!(receipt.available_at, receipt.decided_at);
        assert_eq!(before, snapshot(&pool, id).await);
        let prior: serde_json::Value = checked(
            sqlx::query_scalar("SELECT prior_terminal FROM relay.replay_requests")
                .fetch_one(&pool)
                .await,
            "audit observation",
        );
        assert_eq!(prior["last_error"], "restricted prior evidence");
        assert_eq!(prior["revision"], 4);
        assert_eq!(
            apply(&repo, i.clone()).await,
            ReplayOutcome::Reenabled(receipt.clone())
        );
        let other = ReplayPrincipal::authenticated("another".into(), true, 10);
        assert!(
            replay::outcome(&repo, &other, i.request_id)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            replay::replay(&repo, &other, i.clone()).await.unwrap(),
            ReplayOutcome::IntentConflict
        );
        let mut changed = i.clone();
        changed.reason = "different".into();
        assert_eq!(apply(&repo, changed).await, ReplayOutcome::IntentConflict);
        let owned = repo
            .acquire(
                AcquireRequest::new(
                    reliable_event_relay::domain::delivery::LeaseDuration::from_micros(60_000_000)
                        .unwrap(),
                )
                .targeting(id)
                .unwrap(),
            )
            .await?
            .unwrap();
        assert_eq!(owned.snapshot().attempt_count(), 8);
        assert_eq!(
            repo.complete(owned.key()).await?,
            TransitionOutcome::Applied
        );
        assert_eq!(
            apply(&repo, i.clone()).await,
            ReplayOutcome::Reenabled(receipt.clone())
        );
        assert_eq!(
            replay::outcome(&repo, &principal(), i.request_id)
                .await
                .unwrap(),
            Some(receipt)
        );
        assert!(
            sqlx::query("UPDATE relay.replay_requests SET actor='tampered'")
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("DELETE FROM relay.replay_requests")
                .execute(&pool)
                .await
                .is_err()
        );
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_request_identity_and_same_revision() {
    isolated(|pool| async move {
        setup(&pool).await;
        let id = seed(&pool).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let i = intent(id);
        let (a, b) = tokio::join!(apply(&repo, i.clone()), apply(&repo, i.clone()));
        assert_eq!(a, b);
        let id = seed(&pool).await;
        let a = intent(id);
        let b = intent(id);
        let (a, b) = tokio::join!(apply(&repo, a), apply(&repo, b));
        assert!(matches!(
            (&a, &b),
            (ReplayOutcome::Reenabled(_), ReplayOutcome::StateConflict)
                | (ReplayOutcome::StateConflict, ReplayOutcome::Reenabled(_))
        ));
        let count: i64 = checked(
            sqlx::query_scalar("SELECT count(*) FROM relay.replay_requests")
                .fetch_one(&pool)
                .await,
            "receipt count",
        );
        assert_eq!(count, 2);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rejects_invalid_states_content_and_capacity_without_mutation() {
    isolated(|pool|async move{
 setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());
 for sql in ["event_type=' '","aggregate_type=' '","attempt_count=2147483647","retry_cycle=2147483647","state_revision=9223372036854775807","status='pending',terminal_at=NULL,terminal_reason=NULL","status='processed',processed_at=clock_timestamp(),terminal_at=NULL,terminal_reason=NULL"] {
  let id=seed(&pool).await;checked(sqlx::query(&format!("UPDATE relay.outbox_events SET {sql} WHERE id=$1")).bind(id).execute(&pool).await,"invalid eligibility fixture");let mut i=intent(id);if sql.starts_with("retry_cycle="){i.expected_cycle=i32::MAX;}
if sql.starts_with("state_revision="){i.expected_revision=i64::MAX;}
  let before:serde_json::Value=checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await,"before");
  let result=replay::replay(&repo,&principal(),i).await;assert!(!matches!(result,Ok(ReplayOutcome::Reenabled(_))));let after:serde_json::Value=checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await,"after");assert_eq!(before,after);
 }
 let id=seed(&pool).await;let mut i=intent(id);i.expected_revision=3;assert_eq!(apply(&repo,i).await,ReplayOutcome::StateConflict);
 let count:i64=checked(sqlx::query_scalar("SELECT count(*) FROM relay.replay_requests").fetch_one(&pool).await,"no receipts");assert_eq!(count,0);Ok(())}).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL fault proxy"]
async fn uncertain_commit_and_cancelled_result_have_independent_receipts() {
    isolated(|pool|async move{
 setup(&pool).await;let id=seed(&pool).await;let i=intent(id);let (proxy,task)=faults::lost_commit_ack_pool(&pool).await;let repo=PostgresOutboxRepository::new(proxy.clone());let result=replay::replay(&repo,&principal(),i.clone()).await;
 assert!(matches!(result,Err(replay::ReplayError::Persistence(e)) if e.kind()==PersistenceErrorKind::CommitUncertain));assert!(task.await.unwrap());proxy.close().await;
 let repo=PostgresOutboxRepository::new(pool.clone());let receipt=replay::outcome(&repo,&principal(),i.request_id).await.unwrap().unwrap();assert_eq!(apply(&repo,i).await,ReplayOutcome::Reenabled(receipt));
 let id=seed(&pool).await;let i=intent(id);let (proxy,proxy_task,committed)=faults::withheld_commit_ack_pool(&pool).await;let proxied=PostgresOutboxRepository::new(proxy.clone());let submitted=i.clone();let task=tokio::spawn(async move{replay::replay(&proxied,&principal(),submitted).await});committed.await.unwrap();task.abort();let _=task.await;proxy_task.abort();let _=proxy_task.await;proxy.close().await;
 assert!(replay::outcome(&repo,&principal(),i.request_id).await.unwrap().is_some());Ok(())}).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rejected_transaction_and_lock_wait_recheck() {
    isolated(|pool|async move{
 setup(&pool).await;let id=seed(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());
 checked(sqlx::raw_sql("CREATE FUNCTION relay.reject_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture' USING ERRCODE='23514'; END; $$; CREATE TRIGGER reject_fixture BEFORE INSERT ON relay.replay_requests FOR EACH ROW EXECUTE FUNCTION relay.reject_fixture();").execute(&pool).await,"rejection fixture");
 let i=intent(id);assert!(matches!(replay::replay(&repo,&principal(),i.clone()).await,Err(replay::ReplayError::Persistence(e)) if e.kind()==PersistenceErrorKind::OperationFailed));let row=checked(sqlx::query("SELECT status,state_revision FROM relay.outbox_events WHERE id=$1").bind(id).fetch_one(&pool).await,"rollback");assert_eq!(row.get::<String,_>("status"),"dead_letter");assert_eq!(row.get::<i64,_>("state_revision"),4);
 checked(sqlx::raw_sql("DROP TRIGGER reject_fixture ON relay.replay_requests; DROP FUNCTION relay.reject_fixture();").execute(&pool).await,"remove rejection fixture");
 let mut lock=pool.begin().await?;sqlx::query("SELECT id FROM relay.outbox_events WHERE id=$1 FOR UPDATE").bind(id).execute(&mut *lock).await?;
 let submitted=i.clone();let repo2=repo.clone();let task=tokio::spawn(async move{apply(&repo2,submitted).await});
 tokio::time::timeout(std::time::Duration::from_secs(5), async {
  loop {
   let waiting:bool=checked(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock')").fetch_one(&mut *lock).await,"observe blocked replay");
   if waiting { break; }
   tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }
 }).await.expect("replay entered lock wait");
 sqlx::query("UPDATE relay.outbox_events SET state_revision=5 WHERE id=$1").bind(id).execute(&mut *lock).await?;lock.commit().await?;assert_eq!(task.await.unwrap(),ReplayOutcome::StateConflict);
 assert!(replay::outcome(&repo,&principal(),i.request_id).await.unwrap().is_none());Ok(())}).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn supported_interpretation_boundary_delay_and_maintenance_contention() {
    isolated(|pool|async move{
 setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());let id=seed(&pool).await;
 // Unsupported storage version and unsafe tuple are rejected, never repaired.
 checked(sqlx::raw_sql("ALTER TABLE relay.outbox_events DROP CONSTRAINT ck_outbox_events_schema_version_positive; ALTER TABLE relay.outbox_events DROP CONSTRAINT ck_outbox_events_ownership;").execute(&pool).await,"test-only constraints");
 for fixture in ["schema_version=4294967296","schema_version=1,acquired_at=clock_timestamp()"] {
 checked(sqlx::query(&format!("UPDATE relay.outbox_events SET {fixture} WHERE id=$1")).bind(id).execute(&pool).await,"unsupported fixture");
 assert!(!matches!(replay::replay(&repo,&principal(),intent(id)).await,Ok(ReplayOutcome::Reenabled(_))));
 }
 checked(sqlx::query("UPDATE relay.outbox_events SET acquired_at=NULL WHERE id=$1").bind(id).execute(&pool).await,"restore test fixture");
 let mut i=intent(id);i.delay_ms=5000;
 let ReplayOutcome::Reenabled(receipt)=apply(&repo,i).await else{panic!("delay")};assert_eq!(receipt.available_at-receipt.decided_at,chrono::Duration::seconds(5));
 assert!(repo.acquire(AcquireRequest::new(reliable_event_relay::domain::delivery::LeaseDuration::from_micros(1_000_000).unwrap()).targeting(id).unwrap()).await?.is_none());
 assert!(matches!(repo.preserve_exhausted(ExhaustedEvent::new(id,1,4).unwrap()).await?,RetryTransition::Conflict));
 assert!(repo.discover_candidate(DiscoveryCursor::default()).await?.is_some());
 // Near exhausted lifetime still permits one fresh acquisition without reset.
 let id=seed(&pool).await;checked(sqlx::query("UPDATE relay.outbox_events SET attempt_count=2147483646 WHERE id=$1").bind(id).execute(&pool).await,"near capacity");let mut i=intent(id);i.budget=1;
 assert!(matches!(apply(&repo,i).await,ReplayOutcome::Reenabled(_)));
 let owned=repo.acquire(AcquireRequest::new(reliable_event_relay::domain::delivery::LeaseDuration::from_micros(60_000_000).unwrap()).targeting(id).unwrap()).await?.unwrap();assert_eq!(owned.snapshot().attempt_count(),i32::MAX as u32);assert_eq!(repo.complete(owned.key()).await?,TransitionOutcome::Applied);
 Ok(())}).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_changed_intent_and_deferred_commit_rejection() {
    isolated(|pool|async move{
 setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());let id=seed(&pool).await;let a=intent(id);let mut b=a.clone();b.budget=5;
 let (a,b)=tokio::join!(apply(&repo,a),apply(&repo,b));assert!(matches!((a,b),(ReplayOutcome::Reenabled(_),ReplayOutcome::IntentConflict)|(ReplayOutcome::IntentConflict,ReplayOutcome::Reenabled(_))));
 checked(sqlx::raw_sql("CREATE FUNCTION relay.reject_commit_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'secret fixture source' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER reject_commit_fixture AFTER INSERT ON relay.replay_requests DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION relay.reject_commit_fixture();").execute(&pool).await,"deferred rejection fixture");
 let id=seed(&pool).await;let i=intent(id);let result=replay::replay(&repo,&principal(),i.clone()).await;
 assert!(matches!(&result,Err(replay::ReplayError::Persistence(e)) if e.kind()==PersistenceErrorKind::OperationFailed));assert!(!format!("{result:?}").contains("secret"));assert!(replay::outcome(&repo,&principal(),i.request_id).await.unwrap().is_none());
 let preserved:bool=checked(sqlx::query_scalar("SELECT status='dead_letter' AND state_revision=4 AND retry_cycle=1 FROM relay.outbox_events WHERE id=$1").bind(id).fetch_one(&pool).await,"rejected commit atomicity");assert!(preserved);Ok(())}).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn audit_terminal_evidence_canonical_fields_and_revision_limits() {
    isolated(|pool|async move{
 setup(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());let id=seed(&pool).await;
 checked(sqlx::query("UPDATE relay.outbox_events SET attempt_count=10,terminal_reason='acquisition_budget_exhausted',diagnostic_code='acquisition_budget_exhausted',diagnostic_at=terminal_at,failure_evidence='{\"policy_version\":\"adr008-v1\",\"phase\":\"maintenance\"}' WHERE id=$1").bind(id).execute(&pool).await,"terminal evidence");
 let i=intent(id);assert!(matches!(apply(&repo,i.clone()).await,ReplayOutcome::Reenabled(_)));
 let prior:serde_json::Value=checked(sqlx::query_scalar("SELECT prior_terminal FROM relay.replay_requests WHERE request_id=$1").bind(i.request_id).fetch_one(&pool).await,"terminal audit");assert_eq!(prior["terminal_reason"],"acquisition_budget_exhausted");assert_eq!(prior["diagnostic_at"],prior["terminal_at"]);assert_eq!(prior["failure_evidence"]["phase"],"maintenance");assert_eq!(prior["attempt_count"],10);
 let mut changes=Vec::new();let mut changed=i.clone();changed.event_id=Uuid::now_v7();changes.push(changed);let mut changed=i.clone();changed.expected_revision=5;changes.push(changed);let mut changed=i.clone();changed.expected_cycle=2;changes.push(changed);let mut changed=i.clone();changed.delay_ms=1;changes.push(changed);let mut changed=i.clone();changed.ticket=None;changes.push(changed);
 for changed in changes {assert_eq!(apply(&repo,changed).await,ReplayOutcome::IntentConflict);}
 for revision in [i64::MAX-2,i64::MAX-3] {
  let id=seed(&pool).await;checked(sqlx::query("UPDATE relay.outbox_events SET state_revision=$2,retry_cycle=2147483646 WHERE id=$1").bind(id).bind(revision).execute(&pool).await,"revision capacity fixture");let mut i=intent(id);i.expected_revision=revision;i.expected_cycle=i32::MAX-1;i.budget=1;
  let result=apply(&repo,i).await;
  if revision==i64::MAX-2 {assert_eq!(result,ReplayOutcome::CapacityExhausted);} else {
   let ReplayOutcome::Reenabled(receipt)=result else{panic!("capacity boundary")};assert_eq!(receipt.cycle,i32::MAX);
   let owned=repo.acquire(AcquireRequest::new(reliable_event_relay::domain::delivery::LeaseDuration::from_micros(60_000_000).unwrap()).targeting(id).unwrap()).await?.unwrap();assert_eq!(repo.complete(owned.key()).await?,TransitionOutcome::Applied);
  }
 }
 Ok(())}).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn replay_competes_with_stale_discovery_isolation_and_acquisition() {
    isolated(|pool| async move {
        setup(&pool).await;
        let id = seed(&pool).await;
        checked(sqlx::query("UPDATE relay.outbox_events SET status='pending',terminal_at=NULL,terminal_reason=NULL,attempt_count=10 WHERE id=$1").bind(id).execute(&pool).await, "exhausted discovery fixture");
        let repo = PostgresOutboxRepository::new(pool.clone());
        let candidate = repo.discover_candidate(DiscoveryCursor::default()).await?.unwrap().candidate.unwrap();
        assert!(matches!(repo.isolate_candidate(candidate.clone()).await?, RetryTransition::Applied(_)));
        let mut i = intent(id);
        i.expected_revision = 5;
        i.budget = 1;
        let request = || AcquireRequest::new(reliable_event_relay::domain::delivery::LeaseDuration::from_micros(60_000_000).unwrap()).targeting(id).unwrap();
        let (replayed, stale, acquired) = tokio::join!(apply(&repo, i.clone()), repo.isolate_candidate(candidate), repo.acquire(request()));
        let ReplayOutcome::Reenabled(receipt) = replayed else { panic!("one replay cycle required") };
        assert_eq!(stale?, RetryTransition::Conflict);
        let owned = match acquired? {
            Some(owned) => owned,
            None => repo.acquire(request()).await?.unwrap(),
        };
        assert_eq!(owned.retry_state().cycle(), 2);
        assert_eq!(owned.snapshot().attempt_count(), 11);
        assert!(repo.acquire(request()).await?.is_none());
        assert_eq!(repo.complete(owned.key()).await?, TransitionOutcome::Applied);
        assert_eq!(apply(&repo, i).await, ReplayOutcome::Reenabled(receipt));
        let counts: (i64, i32, i32) = checked(sqlx::query_as("SELECT (SELECT count(*) FROM relay.replay_requests),retry_cycle,attempt_count FROM relay.outbox_events WHERE id=$1").bind(id).fetch_one(&pool).await, "independent race observation");
        assert_eq!(counts, (1, 2, 11));
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn replay_down_removes_audit_metadata_without_reversing_reenabled_event() {
    isolated(|pool| async move {
        setup(&pool).await;
        let id = seed(&pool).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        assert!(matches!(apply(&repo, intent(id)).await, ReplayOutcome::Reenabled(_)));
        let before: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await, "before audit removal");
        checked(sqlx::raw_sql(include_str!("../migrations/20261013000000_add_audited_replay.down.sql")).execute(&pool).await, "isolated replay down");
        let after: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await, "down retains reenabled state");
        assert_eq!(before, after);
        let absent: bool = checked(sqlx::query_scalar("SELECT to_regclass('relay.replay_requests') IS NULL AND to_regprocedure('relay.reject_replay_audit_change()') IS NULL").fetch_one(&pool).await, "audit objects removed");
        assert!(absent);
        checked(sqlx::raw_sql(include_str!("../migrations/20261013000000_add_audited_replay.up.sql")).execute(&pool).await, "isolated replay re-up");
        let count: i64 = checked(sqlx::query_scalar("SELECT count(*) FROM relay.replay_requests").fetch_one(&pool).await, "audit history cannot be restored by re-up");
        assert_eq!(count, 0);
        Ok(())
    }).await;
}
