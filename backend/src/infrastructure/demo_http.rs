//! Separate local demo credential; no CORS, cookies, operator actions or raw diagnostics.
//! Credencial local separada; sem CORS, cookies ou ações de operador.
use crate::application::demo::{self, ActionIntent, Actions, Error, Intent, Repository};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
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
pub fn router<R: Repository + Actions + 'static>(
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
    let loopback_origin = origin.replacen("http://localhost:", "http://127.0.0.1:", 1);
    let permits = Arc::new(tokio::sync::Semaphore::new(2));
    let rate = Arc::new(Mutex::new((Instant::now(), 0u32)));
    Ok(Router::new()
        .route("/demo/runs", post(submit::<R>).get(eligible::<R>))
        .route("/demo/actions", post(action::<R>))
        .route("/demo/history", get(history::<R>))
        .route("/demo/config",get(||async {
            let value=|key:&str,default:&str| std::env::var(key).unwrap_or_else(|_|default.into());
            Json(serde_json::json!({"database":value("POSTGRES_DB","reliable_event_relay"),"database_user":value("POSTGRES_USER",""),"database_host":"localhost","database_port":value("DEMO_DB_PORT","5433"),"management_url":format!("http://localhost:{}",value("DEMO_MANAGEMENT_PORT","15672")),"vhost":value("RABBITMQ_DEFAULT_VHOST","relay"),"exchange":value("RABBITMQ_EXCHANGE","relay.events"),"queue":value("RABBITMQ_QUEUE","relay.local"),"routing_key":value("RABBITMQ_ROUTING_KEY","event")}))
        }))
        .route("/demo/runs/{request_id}", get(evidence::<R>))
        .layer(DefaultBodyLimit::max(1024))
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let permits = permits.clone();
                let rate = rate.clone();
                let token = token.clone();
                let origin = origin.clone();
                let loopback_origin = loopback_origin.clone();
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
                        .is_some_and(|v| v.as_bytes() != origin.as_bytes() && v.as_bytes() != loopback_origin.as_bytes())
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

async fn action<R: Actions>(
    State(repo): State<Arc<R>>,
    Json(intent): Json<ActionIntent>,
) -> Response {
    match repo.submit_action(intent).await {
        Ok(a) => (StatusCode::ACCEPTED, Json(a)).into_response(),
        Err(e) => error(e),
    }
}

async fn eligible<R: Repository>(State(repo): State<Arc<R>>) -> Response {
    match repo.eligible_runs().await {
        Ok(runs) => Json(runs).into_response(),
        Err(e) => error(e),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    limit: u32,
    offset: u32,
}
async fn history<R: Repository>(
    State(repo): State<Arc<R>>,
    Query(page): Query<HistoryQuery>,
) -> Response {
    if !(1..=20).contains(&page.limit) || page.offset > 100 {
        return error(Error::Invalid);
    }
    match repo.retained_runs(page.limit, page.offset).await {
        Ok(runs) => Json(runs).into_response(),
        Err(e) => error(e),
    }
}
