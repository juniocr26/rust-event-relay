//! Separate local demo credential; no CORS, cookies, operator actions or raw diagnostics.
//! Credencial local separada; sem CORS, cookies ou ações de operador.
use crate::application::demo::{self, Error, Intent, Repository};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;
pub fn router<R: Repository + 'static>(
    repository: R,
    token: String,
    origin: String,
) -> Result<Router, &'static str> {
    if token.len() != 64
        || !token.bytes().all(|b| b.is_ascii_hexdigit())
        || !origin.starts_with("http://localhost:")
    {
        return Err("invalid demo credentials/origin (redacted)");
    }
    let permits = Arc::new(tokio::sync::Semaphore::new(2));
    let rate = Arc::new(Mutex::new((Instant::now(), 0u32)));
    Ok(Router::new()
        .route("/demo/runs", post(submit::<R>))
        .route("/demo/runs/{request_id}", get(evidence::<R>))
        .layer(DefaultBodyLimit::max(1024))
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let permits = permits.clone();
                let rate = rate.clone();
                let token = token.clone();
                let origin = origin.clone();
                async move {
                    let headers = req.headers();
                    let supplied = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.strip_prefix("Bearer "))
                        .unwrap_or("");
                    if headers.get_all("authorization").iter().count() != 1
                        || supplied.len() != 64
                        || !bool::from(supplied.as_bytes().ct_eq(token.as_bytes()))
                    {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    if headers
                        .get("origin")
                        .is_some_and(|v| v.as_bytes() != origin.as_bytes())
                        || headers.get("x-demo-client").and_then(|v| v.to_str().ok())
                            != Some("demo61")
                    {
                        return StatusCode::FORBIDDEN.into_response();
                    }
                    {
                        let mut window = rate.lock().expect("rate lock");
                        if window.0.elapsed() >= Duration::from_secs(1) {
                            *window = (Instant::now(), 0);
                        }
                        if window.1 >= 10 {
                            return StatusCode::TOO_MANY_REQUESTS.into_response();
                        }
                        window.1 += 1;
                    }
                    let Ok(_permit) = permits.try_acquire_owned() else {
                        return StatusCode::SERVICE_UNAVAILABLE.into_response();
                    };
                    match tokio::time::timeout(Duration::from_secs(8), next.run(req)).await {
                        Ok(response) => response,
                        Err(_) => (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(
                                serde_json::json!({"error":"outcome_unknown_recover_same_request"}),
                            ),
                        )
                            .into_response(),
                    }
                }
            },
        ))
        .with_state(Arc::new(repository)))
}
fn error(error: Error) -> Response {
    let (status, code) = match error {
        Error::Rejected => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "producer_transaction_rejected",
        ),
        Error::Invalid => (StatusCode::BAD_REQUEST, "unsupported_intent"),
        Error::Conflict => (StatusCode::CONFLICT, "request_intent_conflict"),
        Error::Capacity => (StatusCode::TOO_MANY_REQUESTS, "demo_capacity_exhausted"),
        Error::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable_recover_same_request",
        ),
        Error::CommitUnknown => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commit_unknown_recover_same_request",
        ),
    };
    (status, Json(serde_json::json!({"error":code}))).into_response()
}
async fn submit<R: Repository>(State(repo): State<Arc<R>>, Json(intent): Json<Intent>) -> Response {
    match demo::submit(repo.as_ref(), intent).await {
        Ok(e) => (StatusCode::ACCEPTED, Json(e)).into_response(),
        Err(e) => error(e),
    }
}
async fn evidence<R: Repository>(State(repo): State<Arc<R>>, Path(id): Path<Uuid>) -> Response {
    match repo.evidence(id).await {
        Ok(Some(e)) => Json(e).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => error(e),
    }
}
