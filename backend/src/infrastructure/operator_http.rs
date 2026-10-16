//! Local bearer-credential boundary. No cookies, client actor assertions or payload output.
use crate::{
    application::replay::{self, ReplayError, ReplayPrincipal},
    persistence::{OutboxReplay, PersistenceErrorKind, ReplayIntent, ReplayOutcome},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    token: String,
    actor: String,
    allow_replay: bool,
    maximum_budget: i32,
}
/// No Debug implementation: credentials must never be formatted.
pub struct OperatorCredentials(Vec<Credential>);
impl OperatorCredentials {
    /// Trusted deployment configuration, at most 16 distinct 256-bit hex tokens.
    /// Mint tokens from an OS CSPRNG; configuration ownership is the trust root.
    pub fn parse(json: &str) -> Result<Self, &'static str> {
        let invalid = "invalid replay credential configuration (source redacted)";
        if json.len() > 8192 {
            return Err(invalid);
        }
        let entries: Vec<Credential> = serde_json::from_str(json).map_err(|_| invalid)?;
        if entries.is_empty() || entries.len() > 16 {
            return Err(invalid);
        }
        for (i, c) in entries.iter().enumerate() {
            if c.token.len() != 64
                || !c
                    .token
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || c.actor.is_empty()
                || c.actor.len() > 64
                || !c
                    .actor
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                || !(1..=100).contains(&c.maximum_budget)
                || entries[..i]
                    .iter()
                    .any(|p| p.token == c.token || p.actor == c.actor)
            {
                return Err(invalid);
            }
        }
        Ok(Self(entries))
    }
    fn authenticate(&self, headers: &HeaderMap) -> Result<ReplayPrincipal, StatusCode> {
        if headers.get_all("authorization").iter().count() != 1 {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if token.len() != 64 {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let mut found = None;
        for c in &self.0 {
            if bool::from(c.token.as_bytes().ct_eq(token.as_bytes())) {
                found = Some(ReplayPrincipal::authenticated(
                    c.actor.clone(),
                    c.allow_replay,
                    c.maximum_budget,
                ));
            }
        }
        found.ok_or(StatusCode::UNAUTHORIZED)
    }
}
struct OperatorState<R> {
    repository: R,
    credentials: OperatorCredentials,
}
/// Merge with the existing liveness router. Privileged routes must never have CORS.
pub fn router<R: OutboxReplay + 'static>(
    repository: R,
    credentials: OperatorCredentials,
) -> Router {
    router_observed(
        repository,
        credentials,
        Arc::new(crate::application::observation::Noop),
    )
}
pub fn router_observed<
    R: OutboxReplay + 'static,
    O: crate::application::observation::Observer + Send + 'static,
>(
    repository: R,
    credentials: OperatorCredentials,
    observer: Arc<O>,
) -> Router {
    let admission = Arc::new(tokio::sync::Semaphore::new(2));
    Router::new()
        .route("/operator/replays", post(submit::<R>))
        .route("/operator/replays/{request_id}", get(lookup::<R>))
        .layer(DefaultBodyLimit::max(4096))
        // Outermost boundary: reject before JSON extraction or pool waiting.
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| {
                let admission = admission.clone();
                let observer = observer.clone();
                async move {
                    use crate::application::observation::{Operation as Op, Outcome as Obs};
                    let Ok(_permit) = admission.try_acquire_owned() else {
                        observer.event(Op::Replay, Obs::Saturated);
                        return StatusCode::SERVICE_UNAVAILABLE.into_response();
                    };
                    let observation = crate::application::observation::Observation::new(
                        observer.as_ref(),
                        Op::Replay,
                    );
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        next.run(request),
                    )
                    .await
                    {
                        Ok(response) => {
                            observation.finish(if response.status().is_success() {
                                Obs::Completed
                            } else {
                                Obs::Rejected
                            });
                            response
                        }
                        Err(_) => {
                            observation.finish(Obs::Deadline);
                            StatusCode::SERVICE_UNAVAILABLE.into_response()
                        }
                    }
                }
            },
        ))
        .with_state(Arc::new(OperatorState {
            repository,
            credentials,
        }))
}
fn reject(status: StatusCode) -> Response {
    tracing::warn!(
        status = status.as_u16(),
        "operator request rejected; details redacted"
    );
    status.into_response()
}
fn error(error: ReplayError) -> Response {
    tracing::warn!("operator request failed; details redacted");
    let (status, code) = match error {
        ReplayError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
        ReplayError::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
        ReplayError::Persistence(e) => {
            tracing::warn!(kind=?e.kind(), "operator persistence failure");
            match e.kind() {
                PersistenceErrorKind::CommitUncertain => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "commit_uncertain_inspect_same_request",
                ),
                PersistenceErrorKind::InvalidStoredData => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "stored_event_requires_review",
                ),
                _ => (StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
            }
        }
    };
    (status, Json(serde_json::json!({"error":code}))).into_response()
}
async fn submit<R: OutboxReplay>(
    State(state): State<Arc<OperatorState<R>>>,
    headers: HeaderMap,
    body: Result<Json<ReplayIntent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let principal = match state.credentials.authenticate(&headers) {
        Ok(p) => p,
        Err(s) => {
            tracing::warn!("operator authentication denied");
            return reject(s);
        }
    };
    let intent = match body {
        Ok(Json(i)) => i,
        Err(_) => return reject(StatusCode::BAD_REQUEST),
    };
    match replay::replay(&state.repository, &principal, intent).await {
        Ok(ReplayOutcome::Reenabled(receipt)) => Json(receipt).into_response(),
        Ok(result) => {
            tracing::warn!("operator replay rejected; details redacted");
            let (status, code) = match result {
                ReplayOutcome::IntentConflict => (StatusCode::CONFLICT, "request_intent_conflict"),
                ReplayOutcome::StateConflict => (StatusCode::CONFLICT, "event_state_conflict"),
                ReplayOutcome::CapacityExhausted => {
                    (StatusCode::UNPROCESSABLE_ENTITY, "capacity_exhausted")
                }
                _ => (StatusCode::UNPROCESSABLE_ENTITY, "ineligible"),
            };
            (status, Json(serde_json::json!({"error":code}))).into_response()
        }
        Err(e) => error(e),
    }
}
async fn lookup<R: OutboxReplay>(
    State(state): State<Arc<OperatorState<R>>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let principal = match state.credentials.authenticate(&headers) {
        Ok(p) => p,
        Err(s) => {
            tracing::warn!("operator authentication denied");
            return reject(s);
        }
    };
    let id = match id.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => return reject(StatusCode::BAD_REQUEST),
    };
    match replay::outcome(&state.repository, &principal, id).await {
        Ok(Some(receipt)) => Json(receipt).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => error(e),
    }
}
