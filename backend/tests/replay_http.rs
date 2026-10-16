use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use reliable_event_relay::{
    infrastructure::operator_http::{OperatorCredentials, router},
    persistence::*,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Clone)]
struct Fake(Arc<AtomicUsize>);
impl OutboxReplay for Fake {
    async fn replay(&self, _: AuthorizedReplay) -> Result<ReplayOutcome, PersistenceError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ReplayOutcome::StateConflict)
    }
    async fn replay_outcome(
        &self,
        _: &str,
        _: &str,
        _: Uuid,
    ) -> Result<Option<ReplayReceipt>, PersistenceError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}
fn credentials() -> OperatorCredentials {
    OperatorCredentials::parse(
        &serde_json::json!([
         {"token":"a".repeat(64),"actor":"operator","allow_replay":true,"maximum_budget":10},
         {"token":"b".repeat(64),"actor":"observer","allow_replay":false,"maximum_budget":10}
        ])
        .to_string(),
    )
    .unwrap()
}
#[tokio::test]
async fn actual_bearer_boundary_denies_mutation_repeats_and_lookup() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = router(Fake(calls.clone()), credentials());
    let intent=serde_json::json!({"request_id":Uuid::now_v7(),"event_id":Uuid::now_v7(),"expected_revision":1,"expected_cycle":1,"reason":"fixed route","budget":10,"delay_ms":0,"ticket":null}).to_string();
    for method in ["POST", "GET"] {
        let uri = if method == "POST" {
            "/operator/replays".into()
        } else {
            format!("/operator/replays/{}", Uuid::now_v7())
        };
        for (token, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some("c".repeat(64)), StatusCode::UNAUTHORIZED),
            (Some("b".repeat(64)), StatusCode::FORBIDDEN),
        ] {
            for _ in 0..2 {
                let mut req = Request::builder()
                    .method(method)
                    .uri(&uri)
                    .header("content-type", "application/json");
                if let Some(token) = &token {
                    req = req.header("authorization", format!("Bearer {token}"));
                }
                let response = app
                    .clone()
                    .oneshot(req.body(Body::from(intent.clone())).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), expected);
            }
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let req = Request::builder()
        .method("POST")
        .uri("/operator/replays")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .body(Body::from(intent))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CONFLICT
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let req = Request::builder()
        .method("POST")
        .uri("/operator/replays")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .body(Body::from("x".repeat(4097)))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn configuration_is_bounded_and_has_no_client_privilege_fields() {
    assert!(OperatorCredentials::parse("[]").is_err());
    assert!(OperatorCredentials::parse(&"x".repeat(8193)).is_err());
    assert!(
        OperatorCredentials::parse(
            r#"[{"token":"secret","actor":"admin","allow_replay":true,"maximum_budget":10}]"#
        )
        .is_err()
    );
}

#[tokio::test]
async fn replay_admission_rejects_before_body_read_and_releases_on_cancel() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        infrastructure::operator_http::router_observed,
        telemetry::metrics::{Metrics, Process},
    };
    let metrics = Metrics::new(Process::Http);
    let app = router_observed(
        Fake(Arc::new(AtomicUsize::new(0))),
        credentials(),
        metrics.clone(),
    );
    let stalled = || {
        Request::builder()
            .method("POST")
            .uri("/operator/replays")
            .header("content-type", "application/json")
            .body(Body::from_stream(futures_util::stream::pending::<
                Result<String, std::io::Error>,
            >()))
            .unwrap()
    };
    let mut a = Box::pin(app.clone().oneshot(stalled()));
    let mut b = Box::pin(app.clone().oneshot(stalled()));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    assert_eq!(
        app.clone().oneshot(stalled()).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(metrics.count(Op::Replay, Obs::Saturated), 1);
    drop(a);
    let req = Request::builder()
        .uri(format!("/operator/replays/{}", Uuid::now_v7()))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    drop(b);
    // Error completion also releases the slot.
    assert_eq!(
        app.oneshot(
            Request::builder()
                .uri("/operator/replays/invalid")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
