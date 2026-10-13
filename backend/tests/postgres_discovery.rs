//! All corrupt fixtures and faults are confined to UUID-named disposable databases.
#[path = "support/faults.rs"]
mod faults;
mod support;
use reliable_event_relay::{
    application::{
        delivery::{DeliveryOutcome, deliver_once},
        maintenance::{MaintenanceOutcome, maintain_once},
        publisher::{EventPublisher, PublishError},
    },
    domain::{
        delivery::LeaseDuration,
        event::{EventEnvelope, EventError},
        retry::TerminalReason,
    },
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::*,
};
use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;
use support::{checked, isolated};
use uuid::Uuid;
async fn sql(pool: &PgPool, text: &str) {
    checked(sqlx::raw_sql(text).execute(pool).await, "fixture SQL");
}
async fn setup(pool: &PgPool) {
    sql(
        pool,
        include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
    )
    .await;
    sql(
        pool,
        include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),
    )
    .await;
}
async fn seed(pool: &PgPool, id: u128, kind: &str, count: i32) {
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,available_at,attempt_count,last_error) VALUES ($1,$2,'aggregate','original',1,'2000-01-01','{\"n\":18446744073709551617}','2000-01-01',$3,'original history')").bind(Uuid::from_u128(id)).bind(kind).bind(count).execute(pool).await,"seed");
}
async fn state(pool: &PgPool) -> Value {
    checked(
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM relay.outbox_events e")
            .fetch_one(pool)
            .await,
        "snapshot",
    )
}
async fn observation(repo: &PostgresOutboxRepository) -> DiscoveryObservation {
    repo.discover_candidate(DiscoveryCursor::default())
        .await
        .unwrap()
        .unwrap()
}
struct Confirmed;
impl EventPublisher for Confirmed {
    async fn publish(&self, _: &EventEnvelope) -> Result<(), PublishError> {
        Ok(())
    }
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn mixed_candidates_preserve_raw_history_and_allow_independent_delivery() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1, " \t", 3).await;
        seed(&pool, 2, "valid", 10).await;
        seed(&pool, 3, "valid", i32::MAX).await;
        seed(&pool, 4, "valid", 0).await;
        let before = state(&pool).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let mut cursor = DiscoveryCursor::default();
        for reason in [
            TerminalReason::InvalidStoredEnvelope,
            TerminalReason::AcquisitionBudgetExhausted,
            TerminalReason::StorageCounterExhausted,
        ] {
            let MaintenanceOutcome::Isolation(Ok(RetryTransition::Applied(result))) =
                maintain_once(&repo, &mut cursor).await?
            else {
                panic!("isolation expected")
            };
            assert_eq!(result.reason, reason);
            assert_eq!(result.revision, 1);
        }
        let after = state(&pool).await;
        for i in 0..3 {
            for key in [
                "id",
                "payload",
                "event_type",
                "aggregate_type",
                "aggregate_id",
                "schema_version",
                "occurred_at",
                "created_at",
                "attempt_count",
                "retry_cycle",
                "cycle_baseline",
                "cycle_maximum",
                "last_error",
            ] {
                assert_eq!(after[i][key], before[i][key], "preserved {key}");
            }
            assert_eq!(after[i]["status"], "dead_letter");
            assert!(after[i]["processed_at"].is_null());
            assert!(after[i]["ownership_token"].is_null());
            assert_eq!(after[i]["terminal_at"], after[i]["diagnostic_at"]);
        }
        assert_eq!(
            after[0]["failure_evidence"]["validator_version"],
            "envelope-v1"
        );
        assert!(matches!(
            deliver_once(
                &repo,
                &Confirmed,
                AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())
            )
            .await,
            DeliveryOutcome::Published {
                completion: Ok(TransitionOutcome::Applied),
                ..
            }
        ));
        assert_eq!(state(&pool).await[3]["status"], "processed");
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn traversal_progresses_past_unmanageable_rows_and_wraps_changes_and_insertions() {
    isolated(|pool|async move {
    setup(&pool).await;seed(&pool,0,"valid",0).await;seed(&pool,2,"valid",0).await;seed(&pool,4,"valid",0).await;
    let repo=PostgresOutboxRepository::new(pool.clone());let first=observation(&repo).await;
    assert_eq!(first.evidence,CandidateEvidence::RepairRequired);assert!(first.candidate.is_none());
    seed(&pool,1,"valid",0).await;seed(&pool,5,"valid",0).await;
    sql(&pool,"UPDATE relay.outbox_events SET event_type=' ' WHERE id='00000000-0000-0000-0000-000000000002'").await;
    let second=repo.discover_candidate(first.cursor).await?.unwrap();assert_eq!(second.cursor.after,Some(Uuid::from_u128(1)));
    let third=repo.discover_candidate(second.cursor).await?.unwrap();assert_eq!(third.evidence,CandidateEvidence::InvalidEnvelope(EventError::EmptyEventType));
    let fourth=repo.discover_candidate(third.cursor).await?.unwrap();assert_eq!(fourth.cursor.after,Some(Uuid::from_u128(4)));
    assert!(repo.discover_candidate(fourth.cursor).await?.is_none()); // insertion above watermark waits for wrap
    let mut cursor=first.cursor;
    for _ in 0..4 {maintain_once(&repo,&mut cursor).await?;}
    assert!(cursor.after.is_none());
    // Restart/wrap revisits nil metadata; then traversal still reaches the new row.
    for _ in 0..4 {maintain_once(&repo,&mut cursor).await?;}
    assert_eq!(cursor.after,Some(Uuid::from_u128(5)));
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn locks_concurrent_scanners_stale_revision_cycle_and_unversioned_repair_are_safe() {
    isolated(|pool|async move {
    setup(&pool).await;seed(&pool,1," ",0).await;seed(&pool,2," ",0).await;
    let repo=PostgresOutboxRepository::new(pool.clone());let old=observation(&repo).await.candidate.unwrap();
    let mut lock=checked(pool.begin().await,"lock transaction");checked(sqlx::query("SELECT id FROM relay.outbox_events WHERE id=$1 FOR UPDATE").bind(old.expected.id()).fetch_one(&mut *lock).await,"lock row");
    // Nonlocking discovery observes committed state, advances past held row.
    let seen=observation(&repo).await;assert_eq!(seen.cursor.after,Some(old.expected.id()));
    assert_eq!(repo.isolate_candidate(old.clone()).await?,RetryTransition::Conflict);
    let next=repo.discover_candidate(seen.cursor).await?.unwrap().candidate.unwrap();assert_eq!(next.expected.id(),Uuid::from_u128(2));
    checked(lock.rollback().await,"unlock");
    sql(&pool,"UPDATE relay.outbox_events SET retry_cycle=2 WHERE id='00000000-0000-0000-0000-000000000001'").await;
    assert_eq!(repo.isolate_candidate(old).await?,RetryTransition::Conflict);
    let old=observation(&repo).await.candidate.unwrap();sql(&pool,"UPDATE relay.outbox_events SET state_revision=1 WHERE id='00000000-0000-0000-0000-000000000001'").await;
    assert_eq!(repo.isolate_candidate(old).await?,RetryTransition::Conflict);
    let old=observation(&repo).await.candidate.unwrap();sql(&pool,"UPDATE relay.outbox_events SET event_type='repaired' WHERE id='00000000-0000-0000-0000-000000000001'").await;
    assert_eq!(repo.isolate_candidate(old).await?,RetryTransition::Conflict); // writer omitted revision: evidence is reread
    let (a,b)=tokio::join!(repo.isolate_candidate(next.clone()),repo.isolate_candidate(next));
    assert!(matches!((a?,b?),(RetryTransition::Applied(_),RetryTransition::Conflict)|(RetryTransition::Conflict,RetryTransition::Applied(_))));
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn active_final_owner_completion_and_expired_owner_are_distinguished() {
    isolated(|pool|async move {
    setup(&pool).await;seed(&pool,1,"valid",9).await;let repo=PostgresOutboxRepository::new(pool.clone());
    let owned=repo.acquire(AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())).await?.unwrap();
    assert_eq!(observation(&repo).await.evidence,CandidateEvidence::LiveOwner);
    let fabricated=MaintenanceCandidate{expected:ExhaustedEvent::new(owned.key().event_id(),1,1).unwrap(),evidence:CandidateEvidence::Exhausted(TerminalReason::AcquisitionBudgetExhausted)};
    let (completion,isolation)=tokio::join!(repo.complete(owned.key()),repo.isolate_candidate(fabricated));assert_eq!(completion?,TransitionOutcome::Applied);assert_eq!(isolation?,RetryTransition::Conflict);
    seed(&pool,2,"valid",10).await;
    sql(&pool,"UPDATE relay.outbox_events SET ownership_token=id,acquired_at='2000-01-02',expires_at='2000-01-03' WHERE status='pending'").await;
    let old=observation(&repo).await.candidate.unwrap();
    sql(&pool,"UPDATE relay.outbox_events SET acquired_at=clock_timestamp(),expires_at=clock_timestamp()+interval '1 hour' WHERE status='pending'").await;
    assert_eq!(repo.isolate_candidate(old).await?,RetryTransition::Conflict);
    sql(&pool,"UPDATE relay.outbox_events SET acquired_at='2000-01-02',expires_at='2000-01-03' WHERE status='pending'").await;
    assert!(matches!(repo.isolate_candidate(observation(&repo).await.candidate.unwrap()).await?,RetryTransition::Applied(_)));
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn unsupported_versions_timestamps_and_unsafe_metadata_require_repair() {
    isolated(|pool|async move {
    setup(&pool).await;seed(&pool,1,"valid",0).await;seed(&pool,2,"valid",0).await;seed(&pool,3,"valid",0).await;seed(&pool,4,"valid",0).await;
    // Represents external/corrupt historical writes. Only this isolated test DB
    // loses constraints; production migrations and shared schema are untouched.
    sql(&pool,"ALTER TABLE relay.outbox_events DROP CONSTRAINT ck_outbox_events_schema_version_positive; ALTER TABLE relay.outbox_events DROP CONSTRAINT ck_outbox_events_retry_history; ALTER TABLE relay.outbox_events DROP CONSTRAINT ck_outbox_events_ownership").await;
    sql(&pool,"UPDATE relay.outbox_events SET schema_version=4294967296 WHERE id='00000000-0000-0000-0000-000000000001'; UPDATE relay.outbox_events SET occurred_at='infinity' WHERE id='00000000-0000-0000-0000-000000000002'; UPDATE relay.outbox_events SET state_revision=-1 WHERE id='00000000-0000-0000-0000-000000000003'; UPDATE relay.outbox_events SET ownership_token=id WHERE id='00000000-0000-0000-0000-000000000004'").await;
    let before=state(&pool).await;let repo=PostgresOutboxRepository::new(pool.clone());let mut cursor=DiscoveryCursor::default();
    for expected in [CandidateEvidence::UnsupportedRepresentation,CandidateEvidence::UnsupportedRepresentation,CandidateEvidence::RepairRequired,CandidateEvidence::RepairRequired] {
        let seen=repo.discover_candidate(cursor).await?.unwrap();assert_eq!(seen.evidence,expected);assert!(seen.candidate.is_none());cursor=seen.cursor;
    }
    assert_eq!(state(&pool).await,before);
    pool.close().await;assert_eq!(repo.discover_candidate(cursor).await.unwrap_err().kind(),PersistenceErrorKind::Unavailable);
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn rejected_and_uncertain_commits_and_cancelled_ack_are_observed_independently() {
    isolated(|pool|async move {
    setup(&pool).await;seed(&pool,1," ",0).await;let repo=PostgresOutboxRepository::new(pool.clone());let candidate=observation(&repo).await.candidate.unwrap();
    sql(&pool,"CREATE FUNCTION relay.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER UPDATE ON relay.outbox_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION relay.reject_commit()").await;
    let before=state(&pool).await;assert_eq!(repo.isolate_candidate(candidate.clone()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);assert_eq!(state(&pool).await,before);
    sql(&pool,"DROP TRIGGER reject_commit ON relay.outbox_events").await;
    let (proxy_pool,proxy)=faults::lost_commit_ack_pool(&pool).await;
    let faulted=PostgresOutboxRepository::new(proxy_pool.clone());assert_eq!(faulted.isolate_candidate(candidate).await.unwrap_err().kind(),PersistenceErrorKind::CommitUncertain);assert!(proxy.await.unwrap());proxy_pool.close().await;
    assert_eq!(state(&pool).await[0]["status"],"dead_letter");assert!(repo.discover_candidate(DiscoveryCursor::default()).await?.is_none());
    seed(&pool,2," ",0).await;let candidate=observation(&repo).await.candidate.unwrap();
    let (proxy_pool,proxy,observed)=faults::withheld_commit_ack_pool(&pool).await;let faulted=PostgresOutboxRepository::new(proxy_pool.clone());
    let task=tokio::spawn(async move {faulted.isolate_candidate(candidate).await});tokio::time::timeout(Duration::from_secs(5),observed).await.unwrap().unwrap();
    task.abort();let _=task.await;proxy.abort();let _=proxy.await;tokio::time::timeout(Duration::from_secs(5),proxy_pool.close()).await.unwrap();
    assert_eq!(state(&pool).await[1]["status"],"dead_letter");
    Ok(())
 }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn exact_selection_hint_revalidates_without_repairing_unsafe_prefix() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 0, "valid", 0).await;
        seed(&pool, 1, "valid", 0).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        assert_eq!(
            repo.acquire(AcquireRequest::new(
                LeaseDuration::from_micros(30_000_000).unwrap()
            ))
            .await
            .unwrap_err()
            .kind(),
            PersistenceErrorKind::InvalidStoredData
        );
        let first = observation(&repo).await;
        let valid = repo.discover_candidate(first.cursor).await?.unwrap();
        assert_eq!(valid.evidence, CandidateEvidence::Valid);
        let id = valid.cursor.after.unwrap();
        let owned = repo
            .acquire(
                AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())
                    .targeting(id)
                    .unwrap(),
            )
            .await?
            .unwrap();
        assert_eq!(owned.key().event_id(), id);
        assert_eq!(
            repo.complete(owned.key()).await?,
            TransitionOutcome::Applied
        );
        assert_eq!(state(&pool).await[0]["attempt_count"], 0);
        assert!(
            repo.acquire(
                AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())
                    .targeting(id)
                    .unwrap()
            )
            .await?
            .is_none()
        );
        Ok(())
    })
    .await;
}
