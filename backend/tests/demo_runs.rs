//! Real isolated database tests, never operator demo fixtures.
#[path = "support/faults.rs"]
mod faults;
mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use reliable_event_relay::{
    application::demo::{self, Error, Intent, Repository},
    infrastructure::{demo_http, postgres::demo::DemoRepository},
};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
fn intent() -> Intent {
    Intent {
        request_id: Uuid::now_v7(),
        scenario: "normal".into(),
        initial_stock: 10,
        quantity: 5,
    }
}
async fn setup(p: &PgPool) {
    for sql in [
        include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),
        include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),
        include_str!("../migrations/20261013000000_add_audited_replay.up.sql"),
        include_str!("../migrations/20261014000000_add_atomic_consumer.up.sql"),
        include_str!("../migrations/20261017000000_add_demo_runs.up.sql"),
    ] {
        support::checked(sqlx::raw_sql(sql).execute(p).await, "demo test schema");
    }
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn atomic_idempotent_concurrent_and_independent_effect() {
    support::isolated(|p|async move {
    setup(&p).await; let repo=DemoRepository::new(p.clone()); let i=intent();
    let (a,b)=tokio::join!(demo::submit(&repo,i.clone()),demo::submit(&repo,i.clone()));
    let a=a.unwrap(); let b=b.unwrap(); assert_eq!(a.run_id,b.run_id); assert_eq!(a.event_id,b.event_id);
    assert_eq!(a.stock,Some(10)); assert!(a.receipt.is_none()); assert_eq!(a.outbox.status,"pending");
    let count:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM demo.runs),(SELECT count(*) FROM demo.goods_receipts),(SELECT count(*) FROM relay.outbox_events)").fetch_one(&p).await.unwrap();assert_eq!(count,(1,1,1));
    let mut changed=i.clone(); changed.quantity=6; assert!(matches!(demo::submit(&repo,changed).await,Err(Error::Conflict)));
    // Restore exactly the canonical PostgreSQL representation, like relay discovery.
    let value:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(e) - ARRAY['created_at','status','attempt_count','available_at','processed_at','last_error','delivery_token','lease_until','cycle','revision','retry_budget','cycle_attempt_count','last_failure_kind','last_failure_at'] FROM relay.outbox_events e WHERE id=$1").bind(a.event_id).fetch_one(&p).await.unwrap();
    let envelope:reliable_event_relay::domain::event::EventEnvelope=serde_json::from_value(value).unwrap();
    let event=reliable_event_relay::domain::consumer::InventoryEvent::validate(envelope,Some(&a.event_id.to_string())).unwrap();
    use reliable_event_relay::application::consumer::AtomicInventoryProcessor;
    let consumer=reliable_event_relay::domain::consumer::ConsumerName::new("inventory.stock.v1").unwrap();
    reliable_event_relay::infrastructure::postgres::PostgresInventoryProcessor::new(p.clone()).process(&consumer,&event).await.unwrap();
    let after=demo::submit(&repo,i).await.unwrap(); assert_eq!(after.stock,Some(15));assert_eq!(after.receipt.as_ref().unwrap().stock_before,10);assert_eq!(after.receipt.as_ref().unwrap().stock_after,15);assert_eq!(after.outbox.status,"pending");
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rejected_transaction_rolls_back_all_producer_records() {
    support::isolated(|p|async move {
    setup(&p).await;
    sqlx::raw_sql("CREATE FUNCTION demo.reject_test() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test rejection'; END $$; CREATE TRIGGER demo_reject BEFORE INSERT ON relay.outbox_events FOR EACH ROW EXECUTE FUNCTION demo.reject_test();").execute(&p).await.unwrap();
    let repo=DemoRepository::new(p.clone());assert!(matches!(demo::submit(&repo,intent()).await,Err(Error::Rejected)));
    let count:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM demo.runs)+(SELECT count(*) FROM demo.goods_receipts)+(SELECT count(*) FROM consumer.stock)+(SELECT count(*) FROM relay.outbox_events)").fetch_one(&p).await.unwrap();assert_eq!(count,0);
    Ok(())
 }).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn lost_commit_ack_recovers_same_identity() {
    support::isolated(|p| async move {
        setup(&p).await;
        let i = intent();
        let (proxy, task) = faults::lost_commit_ack_pool(&p).await;
        let result = DemoRepository::new(proxy.clone()).submit(i.clone()).await;
        assert!(matches!(result, Err(Error::CommitUnknown)));
        assert!(task.await.unwrap());
        proxy.close().await;
        let repo = DemoRepository::new(p);
        let prior = repo.evidence(i.request_id).await.unwrap().unwrap();
        let repeated = demo::submit(&repo, i).await.unwrap();
        assert_eq!(prior.event_id, repeated.event_id);
        assert_eq!(repeated.stock, Some(10));
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn cancellation_after_server_commit_recovers_without_new_run() {
    support::isolated(|p| async move {
        setup(&p).await;
        let i = intent();
        let (proxy, task, committed) = faults::withheld_commit_ack_pool(&p).await;
        let repo = DemoRepository::new(proxy.clone());
        let same = i.clone();
        let submit = tokio::spawn(async move { repo.submit(same).await });
        committed.await.unwrap();
        submit.abort();
        let _ = submit.await;
        task.abort();
        let _ = task.await;
        proxy.close().await;
        let repo = DemoRepository::new(p);
        let a = repo.evidence(i.request_id).await.unwrap().unwrap();
        let b = demo::submit(&repo, i).await.unwrap();
        assert_eq!(a.run_id, b.run_id);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn authorized_read_and_csrf_boundaries() {
    support::isolated(|p| async move {
        setup(&p).await;
        let repo = DemoRepository::new(p.clone());
        let i = intent();
        let id = i.request_id;
        demo::submit(&repo, i).await.unwrap();
        let token = "a".repeat(64);
        let router =
            demo_http::router(repo, token.clone(), "http://localhost:5174".into()).unwrap();
        for (auth, origin, header, status) in [
            (false, None, false, StatusCode::UNAUTHORIZED),
            (true, Some("http://evil.test"), true, StatusCode::FORBIDDEN),
            (true, None, false, StatusCode::FORBIDDEN),
            (true, Some("http://localhost:5174"), true, StatusCode::OK),
        ] {
            let mut req = Request::builder().uri(format!("/demo/runs/{id}"));
            if auth {
                req = req.header("authorization", format!("Bearer {token}"));
            }
            if let Some(origin) = origin {
                req = req.header("origin", origin);
            }
            if header {
                req = req.header("x-demo-client", "demo61");
            }
            assert_eq!(
                router
                    .clone()
                    .oneshot(req.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status(),
                status
            );
        }
        p.close().await;
        let req = Request::builder()
            .uri(format!("/demo/runs/{id}"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-demo-client", "demo61")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            router.oneshot(req).await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn finite_admission_preserves_pending_work() {
    support::isolated(|p| async move {
        setup(&p).await;
        let repo = DemoRepository::new(p);
        let first = intent();
        let a = demo::submit(&repo, first.clone()).await.unwrap();
        for _ in 0..3 {
            demo::submit(&repo, intent()).await.unwrap();
        }
        assert!(matches!(
            demo::submit(&repo, intent()).await,
            Err(Error::Capacity)
        ));
        assert_eq!(demo::submit(&repo, first).await.unwrap().run_id, a.run_id);
        Ok(())
    })
    .await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn retained_capacity_stops_without_eviction() {
    support::isolated(|p| async move {
        setup(&p).await;
        let repo=DemoRepository::new(p.clone());
        let first=intent();let a=demo::submit(&repo,first.clone()).await.unwrap();
        // Test-only operator terminalization keeps admission free; this is not delivery evidence.
        sqlx::query("UPDATE relay.outbox_events SET status='dead_letter',terminal_at=clock_timestamp(),terminal_reason='legacy_dead_letter'").execute(&p).await.unwrap();
        for _ in 1..100 {
            let e=demo::submit(&repo,intent()).await.unwrap();
            sqlx::query("UPDATE relay.outbox_events SET status='dead_letter',terminal_at=clock_timestamp(),terminal_reason='legacy_dead_letter' WHERE id=$1").bind(e.event_id).execute(&p).await.unwrap();
        }
        assert!(matches!(demo::submit(&repo,intent()).await,Err(Error::Capacity)));
        assert_eq!(demo::submit(&repo,first).await.unwrap().event_id,a.event_id);
        let n:i64=sqlx::query_scalar("SELECT count(*) FROM demo.runs").fetch_one(&p).await.unwrap();assert_eq!(n,100);
        Ok(())
    }).await;
}
