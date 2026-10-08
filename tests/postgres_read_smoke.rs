//! Opt-in Milestone 1.6 smoke check, not the Milestone 1.7 integration suite.
use chrono::{DateTime, Utc};
use reliable_event_relay::{
    infrastructure::postgres::PostgresOutboxRepository,
    persistence::{BatchSize, EligibleRead, OutboxReader},
};
use sqlx::{
    Connection, Executor, PgConnection,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires Docker PostgreSQL variables and CREATE DATABASE privilege"]
async fn isolated_read_smoke() {
    let options = PgConnectOptions::new()
        .host("postgres")
        .port(5432)
        .username(&std::env::var("POSTGRES_USER").expect("POSTGRES_USER"))
        .password(&std::env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD"))
        .database(&std::env::var("POSTGRES_DB").expect("POSTGRES_DB"));
    let mut admin = PgConnection::connect_with(&options)
        .await
        .unwrap_or_else(|_| panic!("smoke admin connection failed"));
    let name = format!("relay_smoke_{}", Uuid::now_v7().simple());
    admin
        .execute(format!("CREATE DATABASE {name}").as_str())
        .await
        .unwrap_or_else(|_| panic!("create isolated database failed"));
    let result = tokio::spawn(run_smoke(options.database(&name))).await;
    let cleanup = admin
        .execute(format!("DROP DATABASE {name} WITH (FORCE)").as_str())
        .await;
    assert!(cleanup.is_ok(), "isolated database cleanup failed");
    assert!(
        matches!(result, Ok(Ok(()))),
        "repository smoke assertions failed"
    );
}

async fn run_smoke(
    options: PgConnectOptions,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let outcome = async {
        sqlx::raw_sql(include_str!("../migrations/20261007000000_create_relay_schema.up.sql")).execute(&pool).await?;
        sqlx::raw_sql(include_str!("../migrations/20261007175358_create_outbox_events.up.sql")).execute(&pool).await?;
        let reader = PostgresOutboxRepository::new(pool.clone());
        let cutoff = DateTime::<Utc>::from_timestamp(1000, 0).unwrap();
        let request = |limit| EligibleRead::new(cutoff, BatchSize::new(limit).unwrap());
        if !reader.read_eligible(request(10)).await?.is_empty() { return Err("empty read".into()); }
        let id = Uuid::from_u128(1);
        let correlation = Uuid::from_u128(2);
        let causation = Uuid::from_u128(3);
        let occurred = DateTime::<Utc>::from_timestamp(123, 456_000).unwrap();
        let payload = serde_json::from_str::<serde_json::Value>(r#"{"nested":[null,42,"hello"],"integer":18446744073709551617,"decimal":0.12345678901234567890123456789,"huge":123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890}"#)?;
        for (row_id, available, status) in [(id, cutoff, "pending"), (Uuid::from_u128(4), cutoff, "pending"), (Uuid::from_u128(5), cutoff + chrono::Duration::seconds(1), "pending"), (Uuid::from_u128(6), cutoff, "dead_letter")] {
            sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,correlation_id,causation_id,payload,attempt_count,available_at,created_at,status) VALUES ($1,' event ',' aggregate ',' id ',4294967295,$2,$3,$4,$5,7,$6,$2,$7)")
                .bind(row_id).bind(occurred).bind(correlation).bind(causation).bind(&payload).bind(available).bind(status).execute(&pool).await?;
        }
        let before: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM relay.outbox_events e").fetch_one(&pool).await?;
        let bounded = reader.read_eligible(request(1)).await?;
        let rows = reader.read_eligible(request(10)).await?;
        if bounded.len() != 1 || rows.len() != 2 || bounded[0] != rows[0] { return Err("cutoff/batch/order".into()); }
        let event = rows[0].event();
        if event.id() != id || event.occurred_at() != occurred || event.payload() != &payload || event.correlation_id() != Some(correlation) || event.causation_id() != Some(causation) || event.schema_version() != u32::MAX || event.event_type().as_str() != " event " || event.aggregate_type() != " aggregate " || event.aggregate_id() != " id " || rows[0].attempt_count() != 7 || rows[0].available_at() != cutoff { return Err("faithful restoration".into()); }
        let after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM relay.outbox_events e").fetch_one(&pool).await?;
        if before != after || reader.read_eligible(request(10)).await? != rows { return Err("read-only repetition".into()); }
        // PostgreSQL accepts infinite timestamps and whitespace-only names.
        for expression in ["occurred_at = 'infinity'", "occurred_at = '294000-01-01'", "available_at = '-infinity'", "event_type = '   '"] {
            sqlx::query(&format!("UPDATE relay.outbox_events SET {expression} WHERE id = $1")).bind(id).execute(&pool).await?;
            let error = reader.read_eligible(request(10)).await.err().ok_or("malformed row accepted")?;
            if error.kind() != reliable_event_relay::persistence::PersistenceErrorKind::InvalidStoredData { return Err("malformed classification".into()); }
            sqlx::query("UPDATE relay.outbox_events SET occurred_at=$2, available_at=$3, payload=$4, event_type=' event ' WHERE id=$1").bind(id).bind(occurred).bind(cutoff).bind(&payload).execute(&pool).await?;
        }
        Ok(())
    }.await;
    pool.close().await;
    outcome
}
