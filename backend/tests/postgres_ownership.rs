//! Opt-in ownership contracts against isolated real PostgreSQL databases.
mod support;
use chrono::{DateTime, Utc};
use reliable_event_relay::{
    domain::delivery::{LeaseDuration, OwnershipToken},
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::{
        AcquireRequest, BatchSize, EligibleRead, OutboxAcquirer, OutboxCompleter, OutboxReader,
        OutboxReleaser, OwnedEventKey, OwnedOutboxEvent, PersistenceErrorKind, TransitionOutcome,
    },
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use support::{checked, isolated};
use uuid::Uuid;

fn request() -> AcquireRequest {
    AcquireRequest::new(LeaseDuration::from_micros(30_000_000).unwrap())
}
async fn sql(pool: &PgPool, sql: &str) {
    checked(sqlx::raw_sql(sql).execute(pool).await, "fixture SQL");
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
async fn seed(pool: &PgPool, id: u128) {
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,correlation_id,causation_id,payload,available_at,created_at,last_error) VALUES ($1,' event ',' aggregate ',' id ',4294967295,'1999-12-31 23:59:59.123456Z',$2,$3,'{\"integer\":18446744073709551617,\"decimal\":0.12345678901234567890123456789}','2000-01-01','2000-01-01','fixture diagnostic')")
        .bind(Uuid::from_u128(id)).bind(Uuid::from_u128(100)).bind(Uuid::from_u128(101))
        .execute(pool).await,"seed envelope");
}
async fn rows(pool: &PgPool) -> Value {
    checked(
        sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY id),'[]') FROM relay.outbox_events e",
        )
        .fetch_one(pool)
        .await,
        "snapshot rows",
    )
}
async fn acquire(repo: &PostgresOutboxRepository) -> OwnedOutboxEvent {
    repo.acquire(request())
        .await
        .unwrap()
        .expect("eligible fixture")
}
fn immutable(mut row: Value) -> Value {
    for key in [
        "ownership_token",
        "acquired_at",
        "expires_at",
        "attempt_count",
        "status",
        "processed_at",
        "state_revision",
    ] {
        row.as_object_mut().unwrap().remove(key);
    }
    row
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn acquisition_eligibility_single_row_counter_and_reader_observation() {
    isolated(|pool| async move {
        setup(&pool).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        assert!(repo.acquire(request()).await?.is_none());
        for id in 1..=5 { seed(&pool,id).await; }
        sql(&pool,"UPDATE relay.outbox_events SET available_at='9999-01-01' WHERE id='00000000-0000-0000-0000-000000000002'; UPDATE relay.outbox_events SET status='processed',processed_at=clock_timestamp() WHERE id='00000000-0000-0000-0000-000000000003'; UPDATE relay.outbox_events SET status='dead_letter',terminal_at=created_at,terminal_reason='legacy_dead_letter' WHERE id='00000000-0000-0000-0000-000000000004'; UPDATE relay.outbox_events SET attempt_count=2147483647 WHERE id='00000000-0000-0000-0000-000000000005'").await;
        let before = rows(&pool).await;
        let acquisition = request(); let token = acquisition.token();
        let owned = repo.acquire(acquisition).await?.unwrap();
        assert_eq!(owned.key().event_id(),Uuid::from_u128(1));
        assert_eq!(owned.lease().token(),token);
        assert_eq!(owned.snapshot().attempt_count(),1);
        assert_eq!(owned.retry_state().revision(),1);
        assert_eq!(owned.lease().expires_at()-owned.lease().acquired_at(),chrono::Duration::seconds(30));
        let after = rows(&pool).await;
        assert_eq!(immutable(before[0].clone()),immutable(after[0].clone()));
        assert_eq!(&before.as_array().unwrap()[1..],&after.as_array().unwrap()[1..]);
        assert!(repo.acquire(request()).await?.is_none());
        let observed = repo.read_eligible(EligibleRead::new(Utc::now(),BatchSize::new(10).unwrap())).await?;
        assert_eq!(observed.len(),2); // leased and exhausted pending rows remain visible
        assert_eq!(observed[0].event(),owned.snapshot().event());
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn release_reacquisition_and_completion_preserve_identity_and_metadata() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let original = rows(&pool).await;
        let first = acquire(&repo).await;
        let wrong = OwnedEventKey::new(first.key().event_id(), OwnershipToken::fresh());
        for key in [
            wrong,
            OwnedEventKey::new(Uuid::from_u128(999), first.key().token()),
        ] {
            assert_eq!(repo.complete(key).await?, TransitionOutcome::OwnershipLost);
            assert_eq!(repo.release(key).await?, TransitionOutcome::OwnershipLost);
        }
        assert_eq!(repo.release(first.key()).await?, TransitionOutcome::Applied);
        assert_eq!(
            repo.release(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.complete(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        let released = rows(&pool).await;
        for field in [
            "ownership_token",
            "acquired_at",
            "expires_at",
            "processed_at",
        ] {
            assert!(released[0][field].is_null());
        }
        assert_eq!(released[0]["status"], "pending");
        assert_eq!(released[0]["attempt_count"], 1);
        assert_eq!(
            immutable(original[0].clone()),
            immutable(released[0].clone())
        );
        let second = acquire(&repo).await;
        assert_ne!(first.key().token(), second.key().token());
        assert_eq!(first.snapshot().event(), second.snapshot().event());
        assert_eq!(second.snapshot().attempt_count(), 2);
        assert_eq!(
            repo.complete(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.complete(second.key()).await?,
            TransitionOutcome::Applied
        );
        for key in [first.key(), second.key()] {
            assert_eq!(repo.complete(key).await?, TransitionOutcome::OwnershipLost);
            assert_eq!(repo.release(key).await?, TransitionOutcome::OwnershipLost);
        }
        let completed = rows(&pool).await;
        assert_eq!(completed[0]["status"], "processed");
        assert_eq!(completed[0]["attempt_count"], 2);
        let processed: DateTime<Utc> = completed[0]["processed_at"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            processed >= second.lease().acquired_at() && processed < second.lease().expires_at()
        );
        for field in ["ownership_token", "acquired_at", "expires_at"] {
            assert!(completed[0][field].is_null());
        }
        assert_eq!(
            immutable(original[0].clone()),
            immutable(completed[0].clone())
        );
        assert!(repo.acquire(request()).await?.is_none());
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn expired_owner_is_rejected_and_recovery_replaces_token() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let first = acquire(&repo).await;
        sql(
            &pool,
            "UPDATE relay.outbox_events SET acquired_at='2000-01-02',expires_at='2000-01-03'",
        )
        .await;
        let expired = rows(&pool).await;
        assert_eq!(
            repo.complete(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(expired, rows(&pool).await);
        let second = acquire(&repo).await;
        assert_eq!(first.snapshot().event(), second.snapshot().event());
        assert_ne!(first.key().token(), second.key().token());
        assert_eq!(second.snapshot().attempt_count(), 2);
        assert_eq!(
            repo.complete(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(first.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(second.key()).await?,
            TransitionOutcome::Applied
        );
        // Terminal events reject even a matching token supplied by their former owner.
        sql(&pool, "UPDATE relay.outbox_events SET status='dead_letter',terminal_at=created_at,terminal_reason='legacy_dead_letter'").await;
        assert_eq!(
            repo.complete(second.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(second.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn concurrent_acquirers_are_disjoint_and_skip_locked_does_not_prove_empty_backlog() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        seed(&pool, 2).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let barrier = barrier.clone();
            let repo = repo.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                repo.acquire(request()).await
            }));
        }
        barrier.wait().await;
        let a = tasks.remove(0).await.unwrap()?.unwrap();
        let b = tasks.remove(0).await.unwrap()?.unwrap();
        assert_ne!(a.key().event_id(), b.key().event_id());
        assert_ne!(a.key().token(), b.key().token());
        assert_eq!(a.snapshot().attempt_count(), 1);
        assert_eq!(b.snapshot().attempt_count(), 1);
        assert_eq!(repo.release(a.key()).await?, TransitionOutcome::Applied);
        assert_eq!(repo.release(b.key()).await?, TransitionOutcome::Applied);
        let mut blocker = checked(pool.begin().await, "begin blocker");
        checked(
            sqlx::query("SELECT id FROM relay.outbox_events WHERE id=$1 FOR UPDATE")
                .bind(Uuid::from_u128(1))
                .fetch_one(&mut *blocker)
                .await,
            "lock first row",
        );
        let next = tokio::time::timeout(Duration::from_secs(1), repo.acquire(request()))
            .await
            .unwrap()?
            .unwrap();
        assert_eq!(next.key().event_id(), Uuid::from_u128(2));
        assert!(
            tokio::time::timeout(Duration::from_secs(1), repo.acquire(request()))
                .await
                .unwrap()?
                .is_none()
        );
        checked(blocker.rollback().await, "unlock first row");
        assert_eq!(acquire(&repo).await.key().event_id(), Uuid::from_u128(1));
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn malformed_selected_data_and_duration_overflow_do_not_mutate_or_skip() {
    isolated(|pool| async move {
        setup(&pool).await;seed(&pool,1).await;seed(&pool,2).await;
        let repo=PostgresOutboxRepository::new(pool.clone());
        for change in ["event_type=' '","occurred_at='infinity'","available_at='-infinity'"] {
            sql(&pool,&format!("UPDATE relay.outbox_events SET {change} WHERE id='00000000-0000-0000-0000-000000000001'")).await;
            let before=rows(&pool).await;
            assert_eq!(repo.acquire(request()).await.unwrap_err().kind(),PersistenceErrorKind::InvalidStoredData);
            assert_eq!(before,rows(&pool).await);
            sql(&pool,"UPDATE relay.outbox_events SET event_type=' event ',occurred_at='1999-12-31 23:59:59.123456Z',available_at='2000-01-01' WHERE id='00000000-0000-0000-0000-000000000001'").await;
        }
        let before=rows(&pool).await;
        assert_eq!(repo.acquire(AcquireRequest::new(LeaseDuration::from_micros(i64::MAX).unwrap())).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
        assert_eq!(before,rows(&pool).await);
        sql(&pool,"UPDATE relay.outbox_events SET attempt_count=2147483647 WHERE id='00000000-0000-0000-0000-000000000001'").await;
        let before=rows(&pool).await;
        assert_eq!(acquire(&repo).await.key().event_id(),Uuid::from_u128(2));
        assert_eq!(before[0],rows(&pool).await[0]);
        assert!(repo.acquire(request()).await?.is_none());
        Ok(())
    }).await;
}

// Database coordination observes the actual lock wait and then the actual expiry.
// Poll intervals are pacing, not an assumption that a fixed sleep proves expiry.
async fn wait_until(blocker: &mut Transaction<'_, Postgres>, predicate: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            checked(
                sqlx::query("SELECT pg_stat_clear_snapshot()")
                    .execute(&mut **blocker)
                    .await,
                "refresh activity snapshot",
            );
            let ready: bool = checked(
                sqlx::query_scalar(predicate)
                    .fetch_one(&mut **blocker)
                    .await,
                "coordination predicate",
            );
            if ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("database coordination deadline");
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn completion_and_release_sample_time_after_lock_wait_expiration() {
    isolated(|pool| async move {
        setup(&pool).await;
        let repo=PostgresOutboxRepository::new(pool.clone());
        for complete in [true,false] {
            sql(&pool,"DELETE FROM relay.outbox_events").await;seed(&pool,1).await;
            let owner=acquire(&repo).await;
            let mut blocker=checked(pool.begin().await,"begin lock holder");
            checked(sqlx::query("UPDATE relay.outbox_events SET expires_at=clock_timestamp()+interval '600 milliseconds' WHERE id=$1").bind(owner.key().event_id()).execute(&mut *blocker).await,"lock and bound expiry");
            let task_repo=repo.clone();let key=owner.key();
            let task=tokio::spawn(async move {if complete {task_repo.complete(key).await} else {task_repo.release(key).await}});
            wait_until(&mut blocker,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock')").await;
            // Prove transaction-start time predates expiry; relying on now() is wrong.
            let started_before:bool=checked(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity a,relay.outbox_events e WHERE a.datname=current_database() AND a.wait_event_type='Lock' AND a.xact_start<e.expires_at)").fetch_one(&mut *blocker).await,"pre-expiry wait");
            assert!(started_before);
            wait_until(&mut blocker,"SELECT clock_timestamp() >= expires_at FROM relay.outbox_events").await;
            checked(blocker.commit().await,"release blocker after expiry");
            let before=rows(&pool).await;
            assert_eq!(tokio::time::timeout(Duration::from_secs(2),task).await.unwrap().unwrap()?,TransitionOutcome::OwnershipLost);
            assert_eq!(before,rows(&pool).await);
        }
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn server_rejected_commit_is_known_failure_and_rolls_back_all_operations() {
    isolated(|pool| async move {
        setup(&pool).await;seed(&pool,1).await;
        let repo=PostgresOutboxRepository::new(pool.clone());
        let trigger="CREATE FUNCTION relay.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER UPDATE ON relay.outbox_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION relay.reject_commit()";
        sql(&pool,trigger).await;
        let before=rows(&pool).await;
        assert_eq!(repo.acquire(request()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
        assert_eq!(before,rows(&pool).await);
        sql(&pool,"DROP TRIGGER reject_commit ON relay.outbox_events; DROP FUNCTION relay.reject_commit()").await;
        let owned=acquire(&repo).await;sql(&pool,trigger).await;
        let before=rows(&pool).await;
        assert_eq!(repo.complete(owned.key()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
        assert_eq!(before,rows(&pool).await);
        assert_eq!(repo.release(owned.key()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
        assert_eq!(before,rows(&pool).await);
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn cancelling_a_lock_wait_leaves_fixture_unchanged_and_pool_usable() {
    isolated(|pool| async move {
        setup(&pool).await;seed(&pool,1).await;
        let repo=PostgresOutboxRepository::new(pool.clone());let owned=acquire(&repo).await;
        let before=rows(&pool).await;
        let mut blocker=checked(pool.begin().await,"begin blocker");
        checked(sqlx::query("SELECT id FROM relay.outbox_events FOR UPDATE").fetch_one(&mut *blocker).await,"hold row lock");
        let task_repo=repo.clone();let key=owned.key();
        let task=tokio::spawn(async move {task_repo.complete(key).await});
        wait_until(&mut blocker,"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock')").await;
        task.abort();assert!(task.await.unwrap_err().is_cancelled());
        checked(blocker.rollback().await,"unlock after cancellation");
        assert_eq!(before,rows(&pool).await);
        assert_eq!(repo.release(owned.key()).await?,TransitionOutcome::Applied);
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn maximum_counter_and_pre_acquisition_time_reject_without_partial_changes() {
    isolated(|pool| async move {
        setup(&pool).await;seed(&pool,1).await;
        let repo=PostgresOutboxRepository::new(pool.clone());
        sql(&pool,"UPDATE relay.outbox_events SET attempt_count=2147483646,cycle_baseline=2147483646").await;
        let owned=acquire(&repo).await;
        assert_eq!(owned.snapshot().attempt_count(),i32::MAX as u32);
        // A future acquired_at models clock reversal before the start of authority.
        sql(&pool,"UPDATE relay.outbox_events SET acquired_at='9998-01-01',expires_at='9999-01-01'").await;
        let before=rows(&pool).await;
        assert_eq!(repo.complete(owned.key()).await?,TransitionOutcome::OwnershipLost);
        assert_eq!(repo.release(owned.key()).await?,TransitionOutcome::OwnershipLost);
        assert!(repo.acquire(request()).await?.is_none());
        assert_eq!(before,rows(&pool).await);
        sql(&pool,"UPDATE relay.outbox_events SET acquired_at=clock_timestamp()-interval '1 second',expires_at=clock_timestamp()+interval '1 hour'").await;
        assert_eq!(repo.release(owned.key()).await?,TransitionOutcome::Applied);
        let before=rows(&pool).await;
        assert!(repo.acquire(request()).await?.is_none());
        assert_eq!(before,rows(&pool).await);
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn acquisition_acknowledgement_does_not_promise_remaining_lease_time() {
    isolated(|pool| async move {
        setup(&pool).await;
        seed(&pool, 1).await;
        let repo = PostgresOutboxRepository::new(pool.clone());
        let owned = repo
            .acquire(AcquireRequest::new(LeaseDuration::from_micros(1).unwrap()))
            .await?
            .unwrap();
        let expired: bool = checked(
            sqlx::query_scalar("SELECT clock_timestamp() >= expires_at FROM relay.outbox_events")
                .fetch_one(&pool)
                .await,
            "observe expiry after acknowledged acquisition",
        );
        assert!(expired);
        assert_eq!(owned.snapshot().attempt_count(), 1);
        assert_eq!(
            repo.complete(owned.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        assert_eq!(
            repo.release(owned.key()).await?,
            TransitionOutcome::OwnershipLost
        );
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn statement_rejection_before_commit_is_not_commit_uncertainty() {
    isolated(|pool| async move {
        setup(&pool).await;seed(&pool,1).await;
        let repo=PostgresOutboxRepository::new(pool.clone());
        sql(&pool,"CREATE FUNCTION relay.reject_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejection' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_update BEFORE UPDATE ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION relay.reject_update()").await;
        let before=rows(&pool).await;
        assert_eq!(repo.acquire(request()).await.unwrap_err().kind(),PersistenceErrorKind::OperationFailed);
        assert_eq!(before,rows(&pool).await);
        sql(&pool,"DROP TRIGGER reject_update ON relay.outbox_events; DROP FUNCTION relay.reject_update()").await;
        let owned=acquire(&repo).await;
        assert_eq!(repo.release(owned.key()).await?,TransitionOutcome::Applied);
        Ok(())
    }).await;
}

use faults::lost_commit_ack_pool;
#[path = "support/faults.rs"]
mod faults;

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn lost_commit_acknowledgement_returns_uncertain_and_durable_state_is_inspected_separately() {
    isolated(|pool| async move {
        setup(&pool).await;
        let direct=PostgresOutboxRepository::new(pool.clone());
        for operation in 0..3 {
            sql(&pool,"DELETE FROM relay.outbox_events").await;seed(&pool,1).await;
            let owner=if operation==0 {None} else {Some(acquire(&direct).await)};
            let (proxy_pool,proxy)=lost_commit_ack_pool(&pool).await;
            let repository=PostgresOutboxRepository::new(proxy_pool.clone());
            let acquisition=request();let token=acquisition.token();
            let error=tokio::time::timeout(Duration::from_secs(5),async {
                match operation {
                    0=>repository.acquire(acquisition).await.unwrap_err(),
                    1=>repository.complete(owner.as_ref().unwrap().key()).await.unwrap_err(),
                    _=>repository.release(owner.as_ref().unwrap().key()).await.unwrap_err(),
                }
            }).await.expect("uncertain commit deadline");
            assert_eq!(error.kind(),PersistenceErrorKind::CommitUncertain);
            assert!(tokio::time::timeout(Duration::from_secs(5),proxy).await.unwrap().unwrap(),"proxy observed server COMMIT completion");
            tokio::time::timeout(Duration::from_secs(5),proxy_pool.close()).await.expect("proxy pool cleanup");
            // This specific fault occurs AFTER server completion. An arbitrary
            // CommitUncertain elsewhere need not imply either commit or rollback.
            let durable=rows(&pool).await;
            assert_eq!(durable[0]["attempt_count"],1);
            assert_eq!(durable[0]["status"],if operation==1 {"processed"} else {"pending"});
            if operation==0 {
                assert_eq!(durable[0]["ownership_token"],token.get().to_string());
                assert!(!durable[0]["acquired_at"].is_null());assert!(!durable[0]["expires_at"].is_null());
                assert!(direct.acquire(request()).await?.is_none());
                sql(&pool,"UPDATE relay.outbox_events SET acquired_at='2000-01-02',expires_at='2000-01-03'").await;
                let recovered=acquire(&direct).await;
                assert_ne!(recovered.key().token(),token);assert_eq!(recovered.snapshot().attempt_count(),2);
            } else {
                for field in ["ownership_token","acquired_at","expires_at"] {assert!(durable[0][field].is_null());}
                let key=owner.as_ref().unwrap().key();
                assert_eq!(direct.complete(key).await?,TransitionOutcome::OwnershipLost);
                assert_eq!(direct.release(key).await?,TransitionOutcome::OwnershipLost);
                assert_eq!(durable[0]["processed_at"].is_null(),operation!=1);
            }
        }
        Ok(())
    }).await;
}
