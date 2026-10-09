mod support;
use chrono::{DateTime, Utc};
use reliable_event_relay::{infrastructure::postgres::PostgresOutboxRepository, persistence::*};
use support::{checked, isolated};

#[tokio::test]
#[ignore = "requires PostgreSQL and CREATE DATABASE"]
async fn migration_preserves_rows_checks_ownership_and_keeps_reader_observational() {
    isolated(|pool| async move {
        checked(sqlx::raw_sql("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload,available_at) VALUES ('00000000-0000-0000-0000-000000000001','test','aggregate','1',1,'2026-01-01Z','{\"n\":42}','2026-01-01Z')").execute(&pool).await,"seed old schema");
        let before: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e").fetch_one(&pool).await,"snapshot old row");
        checked(sqlx::raw_sql(include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql")).execute(&pool).await,"apply ownership migration");
        let after: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) - 'ownership_token' - 'acquired_at' - 'expires_at' FROM relay.outbox_events e").fetch_one(&pool).await,"snapshot migrated row");
        assert_eq!(before, after);
        for set in [
            "ownership_token='00000000-0000-0000-0000-000000000002'",
            "acquired_at='2026-01-01Z'",
            "expires_at='2026-01-02Z'",
            "ownership_token='00000000-0000-0000-0000-000000000002', acquired_at='2026-01-01Z'",
            "ownership_token='00000000-0000-0000-0000-000000000002', expires_at='2026-01-02Z'",
            "acquired_at='2026-01-01Z', expires_at='2026-01-02Z'",
        ] {
            let error = sqlx::query(&format!("UPDATE relay.outbox_events SET {set}")).execute(&pool).await.unwrap_err();
            assert_eq!(error.as_database_error().unwrap().constraint(), Some("ck_outbox_events_ownership"));
        }
        for (token, acquired, expires, status, attempts) in [
            ("00000000-0000-0000-0000-000000000000","2026-01-01Z","2026-01-02Z","pending",1),
            ("00000000-0000-0000-0000-000000000002","2026-01-01Z","2026-01-01Z","pending",1),
            ("00000000-0000-0000-0000-000000000002","2026-01-02Z","2026-01-01Z","pending",1),
            ("00000000-0000-0000-0000-000000000002","-infinity","2026-01-02Z","pending",1),
            ("00000000-0000-0000-0000-000000000002","2026-01-01Z","infinity","pending",1),
            ("00000000-0000-0000-0000-000000000002","2026-01-01Z","2026-01-02Z","pending",0),
            ("00000000-0000-0000-0000-000000000002","2026-01-01Z","2026-01-02Z","processed",1),
            ("00000000-0000-0000-0000-000000000002","2026-01-01Z","2026-01-02Z","dead_letter",1),
        ] {
            let error = sqlx::query("UPDATE relay.outbox_events SET ownership_token=$1::text::uuid, acquired_at=$2::text::timestamptz, expires_at=$3::text::timestamptz, status=$4, attempt_count=$5, processed_at=CASE WHEN $4='processed' THEN '2026-01-02Z'::timestamptz END")
                .bind(token).bind(acquired).bind(expires).bind(status).bind(attempts).execute(&pool).await.unwrap_err();
            assert_eq!(error.as_database_error().unwrap().constraint(), Some("ck_outbox_events_ownership"));
        }
        checked(sqlx::raw_sql("UPDATE relay.outbox_events SET attempt_count=1,ownership_token='00000000-0000-0000-0000-000000000002',acquired_at='2026-01-01Z',expires_at='2026-01-02Z'").execute(&pool).await,"valid lease");
        let reader = PostgresOutboxRepository::new(pool.clone());
        let request = EligibleRead::new("2026-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap(), BatchSize::new(1).unwrap());
        let stored: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e").fetch_one(&pool).await,"before reader");
        let rows = reader.read_eligible(request).await?;
        assert_eq!(rows.len(),1); assert_eq!(rows[0].attempt_count(),1);
        assert_eq!(rows[0].event().id().to_string(),"00000000-0000-0000-0000-000000000001");
        let read_after: serde_json::Value = checked(sqlx::query_scalar("SELECT to_jsonb(e) FROM relay.outbox_events e").fetch_one(&pool).await,"after reader");
        assert_eq!(stored,read_after);
        checked(sqlx::raw_sql("UPDATE relay.outbox_events SET ownership_token=NULL,acquired_at=NULL,expires_at=NULL,status='processed',processed_at='2026-01-02Z'").execute(&pool).await,"valid terminal");
        assert!(reader.read_eligible(request).await?.is_empty());
        checked(sqlx::raw_sql(include_str!("../migrations/20261009000000_add_delivery_ownership.down.sql")).execute(&pool).await,"rollback isolated migration");
        let id: uuid::Uuid = checked(sqlx::query_scalar("SELECT id FROM relay.outbox_events").fetch_one(&pool).await,"identity after rollback");
        assert_eq!(id.to_string(),"00000000-0000-0000-0000-000000000001");
        Ok(())
    }).await;
}
