#[path = "support/faults.rs"]
mod faults;
mod support;
use reliable_event_relay::{
    application::consumer::*,
    domain::{consumer::*, event::EventEnvelope},
    infrastructure::postgres::PostgresInventoryProcessor,
};
use sqlx::PgPool;
use std::time::Duration;
use support::{checked, isolated};
// Assertions compare stable classifications, while dedicated diagnostics tests inspect sources.
trait ClassifiedProcess: AtomicInventoryProcessor {
    async fn process_kind(
        &self,
        consumer: &ConsumerName,
        event: &InventoryEvent,
    ) -> Result<ProcessingOutcome, ProcessingFailureKind> {
        self.process(consumer, event)
            .await
            .map_err(|error| error.kind())
    }
}
impl ClassifiedProcess for PostgresInventoryProcessor {}
const UP: &str = include_str!("../migrations/20261014000000_add_atomic_consumer.up.sql");
const DOWN: &str = include_str!("../migrations/20261014000000_add_atomic_consumer.down.sql");
fn name() -> ConsumerName {
    ConsumerName::new("inventory.stock.v1").unwrap()
}
fn event(quantity: i64) -> InventoryEvent {
    validate(
        EventEnvelope::new(
            "inventory.units_received",
            "stock",
            "item-1",
            1,
            serde_json::json!({"warehouse_id":"local","sku":"SKU-1","quantity":quantity}),
        )
        .unwrap(),
    )
}
fn validate(e: EventEnvelope) -> InventoryEvent {
    let id = e.id().to_string();
    InventoryEvent::validate(e, Some(&id)).unwrap()
}
fn changed(e: &InventoryEvent) -> InventoryEvent {
    validate(
        e.envelope()
            .clone()
            .with_correlation_id(uuid::Uuid::now_v7()),
    )
}
async fn setup_relay(pool: &PgPool) {
    for sql in [
        include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
        include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),
        include_str!("../migrations/20261013000000_add_audited_replay.up.sql"),
    ] {
        checked(sqlx::raw_sql(sql).execute(pool).await, "upgrade");
    }
}
async fn setup(pool: &PgPool) {
    setup_relay(pool).await;
    checked(sqlx::raw_sql(UP).execute(pool).await, "consumer upgrade");
}
async fn units(pool: &PgPool) -> i64 {
    checked(
        sqlx::query_scalar("SELECT COALESCE(sum(units),0)::bigint FROM consumer.stock")
            .fetch_one(pool)
            .await,
        "stock observation",
    )
}
async fn count(pool: &PgPool) -> i64 {
    checked(
        sqlx::query_scalar("SELECT count(*) FROM consumer.receipts")
            .fetch_one(pool)
            .await,
        "receipt observation",
    )
}
fn applied(units: i64) -> ProcessingOutcome {
    ProcessingOutcome::Applied(EffectReceipt {
        stock_after: Stock::new(units).unwrap(),
    })
}
fn duplicate(units: i64) -> ProcessingOutcome {
    ProcessingOutcome::Duplicate(EffectReceipt {
        stock_after: Stock::new(units).unwrap(),
    })
}
async fn placeholder(
    pool: &PgPool,
    e: &InventoryEvent,
) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut tx = pool.begin().await.unwrap();
    checked(sqlx::query("INSERT INTO consumer.receipts(consumer_name,event_id,representation_version,immutable_content,warehouse_id,sku,quantity) VALUES($1,$2,1,$3,'local','SKU-1',$4)").bind(name().as_str()).bind(e.event_id()).bind(serde_json::to_vec(e.envelope()).unwrap()).bind(e.quantity()).execute(&mut *tx).await,"placeholder");
    tx
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn atomic_effect_history_conflict_scopes_and_overflow() {
    isolated(|pool| async move {
        setup(&pool).await;
        checked(
            sqlx::query("INSERT INTO consumer.stock VALUES('local','SKU-1',10)")
                .execute(&pool)
                .await,
            "initial stock",
        );
        let p = PostgresInventoryProcessor::new(pool.clone());
        let a = event(5);
        let b = event(5);
        assert_eq!(p.process_kind(&name(), &a).await, Ok(applied(15)));
        assert_eq!(p.process_kind(&name(), &a).await, Ok(duplicate(15)));
        assert_eq!(p.process_kind(&name(), &b).await, Ok(applied(20)));
        assert_eq!(p.process_kind(&name(), &a).await, Ok(duplicate(15)));
        assert_eq!(
            p.process_kind(&name(), &changed(&a)).await,
            Ok(ProcessingOutcome::IdentityConflict)
        );
        assert_eq!(units(&pool).await, 20);
        assert_eq!(
            p.process_kind(&ConsumerName::new("other").unwrap(), &a)
                .await,
            Ok(applied(25))
        );
        checked(
            sqlx::query("UPDATE consumer.stock SET units=$1")
                .bind(i64::MAX)
                .execute(&pool)
                .await,
            "overflow fixture",
        );
        assert_eq!(
            p.process_kind(&name(), &event(1)).await,
            Ok(ProcessingOutcome::Invalid(InvalidEvent::Overflow))
        );
        assert_eq!(count(&pool).await, 3);
        assert_eq!(units(&pool).await, i64::MAX);
        let relay: i64 = checked(
            sqlx::query_scalar("SELECT count(*) FROM relay.outbox_events")
                .fetch_one(&pool)
                .await,
            "relay independent",
        );
        assert_eq!(relay, 0);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_identical_conflicting_and_distinct_deliveries() {
    isolated(|pool| async move {
        setup(&pool).await;
        let p = PostgresInventoryProcessor::new(pool.clone());
        let a = event(5);
        let n = name();
        let (x, y) = tokio::join!(p.process_kind(&n, &a), p.process_kind(&n, &a));
        assert!(matches!(
            (x, y),
            (
                Ok(ProcessingOutcome::Applied(_)),
                Ok(ProcessingOutcome::Duplicate(_))
            ) | (
                Ok(ProcessingOutcome::Duplicate(_)),
                Ok(ProcessingOutcome::Applied(_))
            )
        ));
        let b = event(5);
        let c = changed(&b);
        let (x, y) = tokio::join!(p.process_kind(&n, &b), p.process_kind(&n, &c));
        assert!(matches!(
            (x, y),
            (
                Ok(ProcessingOutcome::Applied(_)),
                Ok(ProcessingOutcome::IdentityConflict)
            ) | (
                Ok(ProcessingOutcome::IdentityConflict),
                Ok(ProcessingOutcome::Applied(_))
            )
        ));
        let d = event(5);
        let e = event(5);
        let (x, y) = tokio::join!(p.process_kind(&n, &d), p.process_kind(&n, &e));
        let mut receipts = vec![x.unwrap(), y.unwrap()];
        receipts.sort_by_key(|r| match r {
            ProcessingOutcome::Applied(r) => r.stock_after.units(),
            _ => panic!("new event did not apply"),
        });
        assert_eq!(receipts, vec![applied(15), applied(20)]);
        assert_eq!(units(&pool).await, 20);
        assert_eq!(count(&pool).await, 4);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn schema_constraints_completeness_immutability_and_down_up() {
    async fn case(pool: PgPool) -> support::TestResult {
        setup(&pool).await;
        for sql in [
            "INSERT INTO consumer.stock VALUES('', 'sku', 0)",
            "INSERT INTO consumer.stock VALUES('w','sku',-1)",
            "INSERT INTO consumer.stock VALUES('w','bad sku',0)",
        ] {
            assert!(sqlx::query(sql).execute(&pool).await.is_err());
        }
        for (consumer, id, version, content, quantity) in [
            ("", uuid::Uuid::now_v7(), 1_i16, vec![1_u8], 5_i64),
            ("valid", uuid::Uuid::nil(), 1, vec![1], 5),
            ("valid", uuid::Uuid::now_v7(), 2, vec![1], 5),
            ("valid", uuid::Uuid::now_v7(), 1, vec![], 5),
            ("valid", uuid::Uuid::now_v7(), 1, vec![1; 16385], 5),
            ("valid", uuid::Uuid::now_v7(), 1, vec![1], 0),
        ] {
            assert!(sqlx::query("INSERT INTO consumer.receipts(consumer_name,event_id,representation_version,immutable_content,warehouse_id,sku,quantity) VALUES($1,$2,$3,$4,'local','SKU-1',$5)")
            .bind(consumer).bind(id).bind(version).bind(content).bind(quantity).execute(&pool).await.is_err());
        }
        // Satisfy the FK first: rejection must prove receipt completeness itself.
        checked(
            sqlx::query("INSERT INTO consumer.stock VALUES('local','SKU-1',0)")
                .execute(&pool)
                .await,
            "existing stock for completeness",
        );
        let a = event(5);
        let tx = placeholder(&pool, &a).await;
        let error = tx.commit().await.unwrap_err();
        let database = error.as_database_error().expect("server rejection");
        assert_eq!(database.code().as_deref(), Some("23514"));
        assert_eq!(database.message(), "consumer receipt incomplete");
        assert_eq!(count(&pool).await, 0);
        let mut tx = placeholder(&pool, &a).await;
        checked(
            sqlx::query(
                "UPDATE consumer.stock SET units=5 WHERE warehouse_id='local' AND sku='SKU-1'",
            )
            .execute(&mut *tx)
            .await,
            "stock write",
        );
        checked(sqlx::query("UPDATE consumer.receipts SET stock_before=0,stock_after=5,completed_at=clock_timestamp()").execute(&mut *tx).await,"receipt completion");
        checked(tx.commit().await, "completed transaction");
        for sql in [
            "DELETE FROM consumer.receipts",
            "TRUNCATE consumer.receipts",
            "UPDATE consumer.receipts SET stock_after=6",
            "UPDATE consumer.receipts SET completed_at=NULL",
        ] {
            assert!(sqlx::query(sql).execute(&pool).await.is_err());
        }
        checked(sqlx::raw_sql(DOWN).execute(&pool).await, "consumer down");
        checked(sqlx::raw_sql(UP).execute(&pool).await, "consumer re-up");
        assert_eq!(units(&pool).await, 0);
        assert_eq!(count(&pool).await, 0);
        assert_eq!(
            PostgresInventoryProcessor::new(pool.clone())
                .process_kind(&name(), &a)
                .await,
            Ok(applied(5))
        );
        Ok(())
    }
    isolated(case).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn statement_and_deferred_rejections_rollback_both_writes() {
    async fn case(pool: PgPool) -> support::TestResult {
        setup(&pool).await;
        let p = PostgresInventoryProcessor::new(pool.clone());
        let a = event(5);
        checked(sqlx::raw_sql("CREATE FUNCTION consumer.test_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test rejection' USING ERRCODE='23514'; END $$; CREATE TRIGGER test_reject BEFORE UPDATE ON consumer.receipts FOR EACH ROW EXECUTE FUNCTION consumer.test_reject();").execute(&pool).await,"test-only rejection");
        let error = p.process(&name(), &a).await.unwrap_err();
        assert_eq!(error.kind(), ProcessingFailureKind::PreCommit);
        let source = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<sqlx::Error>())
            .expect("typed SQLx rejection source");
        assert_eq!(
            source
                .as_database_error()
                .and_then(|db| db.code())
                .as_deref(),
            Some("23514")
        );
        assert!(!format!("{error} {error:?}").contains("test rejection"));
        assert_eq!(count(&pool).await, 0);
        assert_eq!(units(&pool).await, 0);
        checked(sqlx::raw_sql("DROP TRIGGER test_reject ON consumer.receipts; CREATE CONSTRAINT TRIGGER test_reject AFTER INSERT ON consumer.receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION consumer.test_reject();").execute(&pool).await,"test-only commit rejection");
        let error = p.process(&name(), &a).await.unwrap_err();
        assert_eq!(error.kind(), ProcessingFailureKind::PreCommit);
        let source = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<sqlx::Error>())
            .expect("typed SQLx rejection source");
        assert_eq!(
            source
                .as_database_error()
                .and_then(|db| db.code())
                .as_deref(),
            Some("23514")
        );
        assert!(!format!("{error} {error:?}").contains("test rejection"));
        assert_eq!(count(&pool).await, 0);
        assert_eq!(units(&pool).await, 0);
        Ok(())
    }
    isolated(case).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn stored_representation_validation_and_exact_roundtrip() {
    async fn case(pool: PgPool) -> support::TestResult {
        setup(&pool).await;
        let p = PostgresInventoryProcessor::new(pool.clone());
        let mut nanos = serde_json::to_value(event(5).envelope())?;
        nanos["occurred_at"] = serde_json::json!("2026-10-13T12:00:00.123456789Z");
        let a = validate(serde_json::from_value(nanos.clone())?);
        assert_eq!(p.process_kind(&name(), &a).await, Ok(applied(5)));
        nanos["occurred_at"] = serde_json::json!("2026-10-13T09:00:00.123456789-03:00");
        assert_eq!(
            p.process_kind(&name(), &validate(serde_json::from_value(nanos)?))
                .await,
            Ok(duplicate(5))
        );
        let bytes: Vec<u8> = checked(
            sqlx::query_scalar("SELECT immutable_content FROM consumer.receipts")
                .fetch_one(&pool)
                .await,
            "content observation",
        );
        let restored: EventEnvelope = serde_json::from_slice(&bytes)?;
        assert_eq!(restored, *a.envelope());
        // A reordered JSON object and equivalent UTC instant compare by model semantics.
        let text = serde_json::to_string(a.envelope())?.replace(
            "\"quantity\":5,\"sku\":\"SKU-1\",\"warehouse_id\":\"local\"",
            "\"warehouse_id\":\"local\",\"sku\":\"SKU-1\",\"quantity\":5",
        );
        assert_eq!(
            p.process_kind(&name(), &validate(serde_json::from_str(&text)?))
                .await,
            Ok(duplicate(5))
        );
        for q in ["5.0", "5e0", "9223372036854775808"] {
            let text = text.replace("\"quantity\":5", &format!("\"quantity\":{q}"));
            let e: EventEnvelope = serde_json::from_str(&text)?;
            assert!(InventoryEvent::validate(e.clone(), Some(&e.id().to_string())).is_err());
        }
        // Exact disposable DB only: owner disables immutability to simulate corruption.
        checked(
            sqlx::query("ALTER TABLE consumer.receipts DISABLE TRIGGER receipt_guard")
                .execute(&pool)
                .await,
            "test-only corruption setup",
        );
        for content in [
            b"invalid".to_vec(),
            serde_json::to_vec(event(5).envelope())?,
        ] {
            checked(
                sqlx::query("UPDATE consumer.receipts SET immutable_content=$1")
                    .bind(content)
                    .execute(&pool)
                    .await,
                "corrupt stored content",
            );
            assert_eq!(
                p.process_kind(&name(), &a).await,
                Err(ProcessingFailureKind::InvalidStoredData)
            );
            assert_eq!(units(&pool).await, 5);
        }
        checked(
            sqlx::query("ALTER TABLE consumer.receipts ENABLE TRIGGER receipt_guard")
                .execute(&pool)
                .await,
            "restore guard",
        );
        Ok(())
    }
    isolated(case).await;
}
// Observe an actual PostgreSQL lock wait; no sleep-based scheduling assumptions.
async fn wait_for_lock(pool: &PgPool) {
    async fn observe(pool: &PgPool) {
        loop {
            let waiting: bool = checked(sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND pid<>pg_backend_pid())"
            ).fetch_one(pool).await, "lock observation");
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    }
    tokio::time::timeout(Duration::from_secs(3), observe(pool))
        .await
        .expect("lock wait not observed");
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn waiting_writer_rollback_and_lock_deadline() {
    isolated(|pool| async move {
        setup(&pool).await;
        let a = event(5);
        // Separate pool leaves capacity for independent lock observation.
        let observer = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with((*pool.connect_options()).clone())
            .await?;
        let tx = placeholder(&pool, &a).await;
        let p = PostgresInventoryProcessor::new(pool.clone());
        let copy = a.clone();
        let task = tokio::spawn(async move { p.process_kind(&name(), &copy).await });
        wait_for_lock(&observer).await;
        checked(tx.rollback().await, "first writer rollback");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task).await??,
            Ok(applied(5))
        );
        let b = event(5);
        let tx = placeholder(&pool, &b).await;
        let deadline_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(
                (*pool.connect_options())
                    .clone()
                    .options([("lock_timeout", "100"), ("statement_timeout", "1000")]),
            )
            .await?;
        let p = PostgresInventoryProcessor::new(deadline_pool.clone());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), p.process_kind(&name(), &b)).await?,
            Err(ProcessingFailureKind::PreCommit)
        );
        checked(tx.rollback().await, "deadline holder rollback");
        assert_eq!(count(&pool).await, 1);
        assert_eq!(units(&pool).await, 5);
        deadline_pool.close().await;
        observer.close().await;
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn lost_ack_and_cancellation_after_observed_commit() {
    isolated(|pool| async move {
        setup(&pool).await;
        let a = event(5);
        let (proxy, server) = faults::lost_commit_ack_pool(&pool).await;
        let p = PostgresInventoryProcessor::new(proxy.clone());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), p.process_kind(&name(), &a)).await?,
            Err(ProcessingFailureKind::CommitUncertain)
        );
        assert!(tokio::time::timeout(Duration::from_secs(5), server).await??);
        assert_eq!(units(&pool).await, 5);
        assert_eq!(count(&pool).await, 1);
        let p = PostgresInventoryProcessor::new(pool.clone());
        assert_eq!(p.process_kind(&name(), &a).await, Ok(duplicate(5)));
        proxy.close().await;
        let b = event(5);
        let (proxy, server, observed) = faults::withheld_commit_ack_pool(&pool).await;
        let p = PostgresInventoryProcessor::new(proxy.clone());
        let copy = b.clone();
        let task = tokio::spawn(async move { p.process_kind(&name(), &copy).await });
        tokio::time::timeout(Duration::from_secs(5), observed).await??;
        assert_eq!(units(&pool).await, 10);
        assert_eq!(count(&pool).await, 2);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        server.abort();
        let _ = server.await;
        proxy.close().await;
        assert_eq!(
            PostgresInventoryProcessor::new(pool.clone())
                .process_kind(&name(), &b)
                .await,
            Ok(duplicate(10))
        );
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn cancellation_before_admission_and_while_waiting_leaves_no_effect() {
    isolated(|pool| async move {
        setup(&pool).await;
        let p = PostgresInventoryProcessor::new(pool.clone());
        let a = event(5);
        let n = name();
        let future = p.process_kind(&n, &a);
        drop(future);
        assert_eq!(count(&pool).await, 0);
        let observer = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with((*pool.connect_options()).clone())
            .await?;
        let tx = placeholder(&pool, &a).await;
        let copy = a.clone();
        let task = tokio::spawn(async move { p.process_kind(&name(), &copy).await });
        wait_for_lock(&observer).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        checked(tx.rollback().await, "cancel holder rollback");
        // A new successful call also waits for SQLx's queued rollback/connection recovery.
        assert_eq!(
            PostgresInventoryProcessor::new(pool.clone())
                .process_kind(&name(), &a)
                .await,
            Ok(applied(5))
        );
        assert_eq!(count(&pool).await, 1);
        assert_eq!(units(&pool).await, 5);
        observer.close().await;
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_loser_reads_winners_completed_receipt_after_wait() {
    async fn case(pool: PgPool) -> support::TestResult {
        setup(&pool).await;
        checked(
            sqlx::query("INSERT INTO consumer.stock VALUES('local','SKU-1',10)")
                .execute(&pool)
                .await,
            "seed stock",
        );
        let concurrent = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect_with((*pool.connect_options()).clone())
            .await?;
        let mut blocker = pool.begin().await?;
        checked(
            sqlx::query("SELECT units FROM consumer.stock FOR UPDATE")
                .execute(&mut *blocker)
                .await,
            "hold stock",
        );
        let a = event(5);
        let p = PostgresInventoryProcessor::new(concurrent.clone());
        let copy = a.clone();
        let first = tokio::spawn(async move { p.process_kind(&name(), &copy).await });
        wait_for_lock(&concurrent).await;
        let p = PostgresInventoryProcessor::new(concurrent.clone());
        let copy = a.clone();
        let second = tokio::spawn(async move { p.process_kind(&name(), &copy).await });
        tokio::time::timeout(Duration::from_secs(2),async {
            loop {
                let waits:i64=checked(sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'").fetch_one(&concurrent).await,"observe both writers");
                if waits>=2 {break;
}
                tokio::task::yield_now().await;
            }
        }).await?;
        checked(blocker.rollback().await, "release stock");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), first).await??,
            Ok(applied(15))
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), second).await??,
            Ok(duplicate(15))
        );
        assert_eq!(units(&pool).await, 15);
        assert_eq!(count(&pool).await, 1);
        concurrent.close().await;
        Ok(())
    }
    isolated(case).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn consumer_upgrade_preserves_existing_relay_envelope_and_history() {
    isolated(|pool| async move {
        setup_relay(&pool).await;
        let id = uuid::Uuid::now_v7();
        checked(sqlx::query("INSERT INTO relay.outbox_events(id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,attempt_count,state_revision) VALUES($1,'existing.event','stock','item-1',1,clock_timestamp(),'{\"exact\":123456789012345678901234567890}',7,9)")
            .bind(id).execute(&pool).await, "existing relay fixture");
        let before: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await, "before upgrade");
        checked(sqlx::raw_sql(UP).execute(&pool).await, "consumer upgrade");
        let after: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e WHERE id=$1").bind(id).fetch_one(&pool).await, "after upgrade");
        assert_eq!(before, after);
        assert_eq!(count(&pool).await, 0);
        assert_eq!(units(&pool).await, 0);
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn unavailable_pool_retains_typed_diagnostics_without_mutation() {
    isolated(|pool| async move {
        setup(&pool).await;
        let closed = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy_with((*pool.connect_options()).clone());
        closed.close().await;
        let p = PostgresInventoryProcessor::new(closed);
        let error = p.process(&name(), &event(5)).await.unwrap_err();
        assert_eq!(error.kind(), ProcessingFailureKind::Unavailable);
        assert!(matches!(
            std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<sqlx::Error>()),
            Some(sqlx::Error::PoolClosed)
        ));
        assert_eq!(count(&pool).await, 0);
        assert_eq!(units(&pool).await, 0);
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and test-owned NOLOGIN role creation"]
async fn restricted_consumer_grants_allow_atomic_adapter_without_receipt_removal() {
    isolated(|pool| async move {
        setup(&pool).await;
        // A NOLOGIN UUID role has no password and is never attached to shared data.
        let role = format!("relay_it_role_{}", uuid::Uuid::now_v7().simple());
        checked(sqlx::raw_sql(&format!("CREATE ROLE {role} NOLOGIN; GRANT USAGE ON SCHEMA consumer TO {role}; GRANT SELECT,INSERT,UPDATE(units) ON consumer.stock TO {role}; GRANT SELECT,INSERT,UPDATE(stock_before,stock_after,completed_at) ON consumer.receipts TO {role};")).execute(&pool).await, "test role grants");
        let role_for_connect = role.clone();
        let restricted = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |connection, _| {
                let command = format!("SET ROLE {role_for_connect}");
                Box::pin(async move { sqlx::query(&command).execute(connection).await?; Ok(()) })
            })
            .connect_with((*pool.connect_options()).clone()).await;
        let result = if let Ok(restricted) = &restricted {
            let restricted = restricted.clone();
            Some(tokio::spawn(async move {
                let p = PostgresInventoryProcessor::new(restricted.clone());
                let a = event(5);
                assert_eq!(p.process_kind(&name(), &a).await, Ok(applied(5)));
                assert_eq!(p.process_kind(&name(), &a).await, Ok(duplicate(5)));
                for command in ["DELETE FROM consumer.receipts", "TRUNCATE consumer.receipts", "UPDATE consumer.receipts SET immutable_content=immutable_content", "DROP TABLE consumer.stock"] {
                    let error = sqlx::query(command).execute(&restricted).await.expect_err("restricted authority");
                    assert_eq!(error.as_database_error().and_then(|e|e.code()).as_deref(),Some("42501"));
                }
                assert_eq!(units(&restricted).await,5);assert_eq!(count(&restricted).await,1);
            }).await)
        } else { None };
        // Revoke only this exact role's grants before dropping it, even after panic.
        if let Ok(restricted) = restricted { restricted.close().await; }
        checked(sqlx::raw_sql(&format!("DROP OWNED BY {role}; DROP ROLE {role};")).execute(&pool).await,"test role cleanup");
        let remains:bool=checked(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=$1)").bind(&role).fetch_one(&pool).await,"role cleanup observation");
        assert!(!remains);
        match result {
            Some(Ok(()))=>{},
            Some(Err(error)) if error.is_panic()=>std::panic::resume_unwind(error.into_panic()),
            _=>panic!("restricted role setup/task failed (source redacted)"),
        }
        Ok(())
    }).await;
}
