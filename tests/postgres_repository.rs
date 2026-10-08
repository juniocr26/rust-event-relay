//! Opt-in public-contract tests against real PostgreSQL.
mod support;
use chrono::{DateTime, Utc};
use reliable_event_relay::{
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::{BatchSize, EligibleRead, OutboxReader, PersistenceError, PersistenceErrorKind},
};
use serde_json::Value;
use sqlx::PgPool;
use std::error::Error;
use support::{TestResult, checked, isolated};
use uuid::Uuid;

fn time(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}
fn request(limit: usize) -> EligibleRead {
    EligibleRead::new(time("2026-10-08T12:00:00Z"), BatchSize::new(limit).unwrap())
}
async fn seed(pool: &PgPool, id: u128, available: &str, created: &str, status: &str) {
    checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,available_at,created_at,status,processed_at,last_error) VALUES ($1,' event ',' aggregate ',' id ',1,'1999-12-31 23:59:59.123456+00','null',$2::text::timestamptz,$3::text::timestamptz,$4,CASE WHEN $4='processed' THEN '2026-10-08'::timestamptz END,'fixture diagnostic')").bind(Uuid::from_u128(id)).bind(available).bind(created).bind(status).execute(pool).await, "insert fixture");
}
async fn execute(pool: &PgPool, sql: &str) {
    checked(sqlx::raw_sql(sql).execute(pool).await, "fixture SQL");
}
fn classified(error: &PersistenceError, kind: PersistenceErrorKind) {
    assert_eq!(error.kind(), kind);
    assert!(error.source().is_some());
    // Formatting must remain exactly classification-only even for database sources.
    let expected = PersistenceError::with_source(kind, std::io::Error::other("secret"));
    assert_eq!(
        format!("{error} {error:?}"),
        format!("{expected} {expected:?}")
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn eligibility_bounds_and_adapter_tie_breakers() {
    isolated(|pool| async move {
        let reader = PostgresOutboxRepository::new(pool.clone());
        assert!(reader.read_eligible(request(10)).await?.is_empty());
        for (id, available, created, status) in [
            (9,"2026-10-08 11:00Z","2026-10-07 00:00Z","pending"),
            (3,"2026-10-08 12:00Z","2026-10-07 00:00Z","pending"),
            (2,"2026-10-08 12:00Z","2026-10-07 00:00Z","pending"),
            (1,"2026-10-08 12:00Z","2026-10-07 01:00Z","pending"),
            (4,"2026-10-08 12:00:00.000001Z","2026-10-07 00:00Z","pending"),
            (5,"2026-10-08 11:00Z","2026-10-07 00:00Z","processed"),
            (6,"2026-10-08 11:00Z","2026-10-07 00:00Z","dead_letter")
        ] { seed(&pool,id,available,created,status).await; }
        for (limit, expected) in [(1,vec![9]),(3,vec![9,2,3]),(10,vec![9,2,3,1])] {
            let rows = reader.read_eligible(request(limit)).await?;
            assert!(rows.len() <= limit);
            assert_eq!(rows.iter().map(|r|r.event().id()).collect::<Vec<_>>(), expected.into_iter().map(Uuid::from_u128).collect::<Vec<_>>());
        }
        execute(&pool,"DELETE FROM relay.outbox_events WHERE id IN ('00000000-0000-0000-0000-000000000009','00000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000001')").await;
        assert!(reader.read_eligible(request(10)).await?.is_empty());
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn restores_every_envelope_field_and_pending_metadata() {
    isolated(|pool| async move {
        let payloads = [r#"{"nested":{"items":[null,true,"x"]},"integer":18446744073709551617,"decimal":0.12345678901234567890123456789,"exponent":1e1000}"#, "null", "[1,false,\"x\"]", "42", "\"scalar\"", "true"];
        for (index,payload) in payloads.iter().enumerate() {
            let id = index as u128 + 1;
            seed(&pool,id,"2026-10-08 09:00:00.654321-03","1999-12-31 00:00Z","pending").await;
            checked(sqlx::query("UPDATE relay.outbox_events SET payload=$2::text::jsonb,schema_version=$3,attempt_count=$4,correlation_id=$5,causation_id=$6,occurred_at='1999-12-31 20:59:59.123456-03' WHERE id=$1")
                .bind(Uuid::from_u128(id)).bind(*payload).bind(if index==0 {i64::from(u32::MAX)} else {1}).bind(if index==0 {i32::MAX} else {0}).bind((index==0).then(||Uuid::from_u128(100))).bind((index==0).then(||Uuid::from_u128(101))).execute(&pool).await,"customize fixture");
        }
        let rows = PostgresOutboxRepository::new(pool.clone()).read_eligible(EligibleRead::new(time("2026-10-08T12:00:01Z"),BatchSize::new(10).unwrap())).await?;
        assert_eq!(rows.len(),payloads.len());
        for (index,row) in rows.iter().enumerate() {
            let event = row.event();
            assert_eq!(event.id(),Uuid::from_u128(index as u128+1));
            assert_eq!(event.occurred_at(),time("1999-12-31T23:59:59.123456Z"));
            assert_eq!(event.event_type().as_str()," event ");
            assert_eq!(event.aggregate_type()," aggregate "); assert_eq!(event.aggregate_id()," id ");
            assert_eq!(event.schema_version(),if index==0 {u32::MAX} else {1});
            assert_eq!(row.attempt_count(),if index==0 {i32::MAX as u32} else {0});
            assert_eq!(event.correlation_id(),(index==0).then(||Uuid::from_u128(100)));
            assert_eq!(event.causation_id(),(index==0).then(||Uuid::from_u128(101)));
            assert_eq!(row.available_at(),time("2026-10-08T12:00:00.654321Z"));
            // JSONB normalizes exponent spelling. Compare against the stored semantic value.
            let stored: Value = checked(sqlx::query_scalar("SELECT payload FROM relay.outbox_events WHERE id=$1").bind(event.id()).fetch_one(&pool).await,"read stored payload");
            assert_eq!(event.payload(),&stored);
            if index==0 {
                assert_eq!(stored["integer"].to_string(),"18446744073709551617");
                assert_eq!(stored["decimal"].to_string(),"0.12345678901234567890123456789");
                assert_eq!(stored["exponent"].to_string(),format!("1{}","0".repeat(1000)));
            } else { assert_eq!(&stored,&serde_json::from_str::<Value>(payloads[index]).unwrap()); }
        }
        Ok(())
    }).await;
}

async fn stored_rows(pool: &PgPool) -> Value {
    checked(
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM relay.outbox_events e")
            .fetch_one(pool)
            .await,
        "snapshot complete rows",
    )
}
#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn repeated_and_independent_readers_observe_without_mutation() {
    isolated(|pool| async move {
        seed(
            &pool,
            1,
            "2026-10-08 11:00Z",
            "2026-10-07 00:00Z",
            "pending",
        )
        .await;
        let before = stored_rows(&pool).await;
        let a = PostgresOutboxRepository::new(pool.clone());
        let b = PostgresOutboxRepository::new(pool.clone());
        let barrier = tokio::sync::Barrier::new(2);
        let (first, second) = tokio::join!(
            async {
                barrier.wait().await;
                a.read_eligible(request(10)).await
            },
            async {
                barrier.wait().await;
                b.read_eligible(request(10)).await
            }
        );
        let first = first?;
        assert_eq!(first.len(), 1);
        assert_eq!(first, second?);
        assert_eq!(first, a.read_eligible(request(10)).await?);
        assert_eq!(before, stored_rows(&pool).await);
        Ok(())
    })
    .await;
}

async fn invalid_case(expression: &'static str) {
    isolated(move |pool| async move {
        seed(&pool,1,"2026-10-08 10:00Z","2026-10-07 00:00Z","pending").await;
        seed(&pool,2,"2026-10-08 11:00Z","2026-10-07 00:00Z","pending").await;
        execute(&pool,&format!("UPDATE relay.outbox_events SET {expression} WHERE id='00000000-0000-0000-0000-000000000002'")).await;
        let reader=PostgresOutboxRepository::new(pool.clone());
        let error=reader.read_eligible(request(10)).await.unwrap_err();
        classified(&error,PersistenceErrorKind::InvalidStoredData);
        if expression.contains("infinity") || expression.contains("294000") { assert!(matches!(error.source().unwrap().downcast_ref::<sqlx::Error>(),Some(sqlx::Error::ColumnDecode {..}))); }
        execute(&pool,"UPDATE relay.outbox_events SET event_type=' event ',aggregate_type=' aggregate ',aggregate_id=' id ',occurred_at='1999-12-31',available_at='2026-10-08 11:00Z'").await;
        assert_eq!(reader.read_eligible(request(10)).await?.len(),2);
        Ok(())
    }).await;
}
macro_rules! invalid_test {
    ($name:ident,$expression:literal) => {
        #[tokio::test]
        #[ignore = "requires PostgreSQL and CREATE DATABASE"]
        async fn $name() {
            invalid_case($expression).await;
        }
    };
}
invalid_test!(rejects_blank_event_type, "event_type='   '");
invalid_test!(rejects_blank_aggregate_type, "aggregate_type='   '");
invalid_test!(rejects_blank_aggregate_id, "aggregate_id='   '");
invalid_test!(
    rejects_positive_infinite_occurrence,
    "occurred_at='infinity'"
);
invalid_test!(
    rejects_negative_infinite_occurrence,
    "occurred_at='-infinity'"
);
invalid_test!(
    rejects_selected_negative_infinite_availability,
    "available_at='-infinity'"
);
invalid_test!(
    rejects_finite_timestamp_outside_chrono,
    "occurred_at='294000-01-01'"
);

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn malformed_excluded_rows_do_not_poison_eligible_batch() {
    isolated(|pool| async move {
        for (id,status,available) in [(1,"pending","2026-10-08 11:00Z"),(2,"pending","2026-10-09 00:00Z"),(3,"processed","2026-10-08 11:00Z"),(4,"dead_letter","2026-10-08 11:00Z")] {seed(&pool,id,available,"2026-10-07 00:00Z",status).await;}
        execute(&pool,"UPDATE relay.outbox_events SET event_type='   ',occurred_at='infinity' WHERE id <> '00000000-0000-0000-0000-000000000001'").await;
        let rows=PostgresOutboxRepository::new(pool).read_eligible(request(10)).await?;
        assert_eq!(rows.len(),1); assert_eq!(rows[0].event().id(),Uuid::from_u128(1)); Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn missing_table_preserves_typed_database_failure() {
    isolated(|pool| async move {
        execute(&pool, "DROP TABLE relay.outbox_events").await;
        let error = PostgresOutboxRepository::new(pool)
            .read_eligible(request(1))
            .await
            .unwrap_err();
        classified(&error, PersistenceErrorKind::OperationFailed);
        match error
            .source()
            .unwrap()
            .downcast_ref::<sqlx::Error>()
            .unwrap()
        {
            sqlx::Error::Database(source) => assert_eq!(source.code().as_deref(), Some("42P01")),
            _ => panic!("expected database source"),
        };
        Ok(())
    })
    .await;
}

#[tokio::test]
async fn closed_pool_and_oversized_limit_fail_through_public_contract() -> TestResult {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy_with(sqlx::postgres::PgConnectOptions::new());
    pool.close().await;
    let reader = PostgresOutboxRepository::new(pool);
    let error = reader.read_eligible(request(1)).await.unwrap_err();
    classified(&error, PersistenceErrorKind::Unavailable);
    assert!(matches!(
        error.source().unwrap().downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::PoolClosed)
    ));
    if let Ok(max) = usize::try_from(i64::MAX) {
        let error = reader.read_eligible(request(max + 1)).await.unwrap_err();
        classified(&error, PersistenceErrorKind::OperationFailed);
        assert!(error.source().unwrap().is::<std::num::TryFromIntError>());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
#[should_panic(expected = "intentional fixture assertion")]
async fn harness_cleans_up_after_assertion_failure() {
    isolated(|_pool| async move {
        panic!("intentional fixture assertion");
        #[allow(unreachable_code)]
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
#[should_panic(expected = "integration case returned an error")]
async fn harness_cleans_up_after_returned_error() {
    isolated(|_pool| async move { Err(std::io::Error::other("private fixture details").into()) })
        .await;
}
