//! Authorization and validation are enforced on mutation AND observation paths.
use crate::persistence::{
    AuthorizedReplay, OutboxReplay, PersistenceError, ReplayIntent, ReplayOutcome, ReplayReceipt,
};
use uuid::Uuid;

/// Trusted principal minted by a credential-verifying transport, never request JSON.
#[derive(Clone)]
pub struct ReplayPrincipal {
    actor: String,
    allowed: bool,
    maximum_budget: i32,
}
impl ReplayPrincipal {
    pub fn authenticated(actor: String, allowed: bool, maximum_budget: i32) -> Self {
        Self {
            actor,
            allowed,
            maximum_budget,
        }
    }
    fn authorized(&self) -> bool {
        self.allowed && !self.actor.is_empty() && (1..=100).contains(&self.maximum_budget)
    }
}
/// Global local-operator scope; this project has no tenant model.
pub const REPLAY_SCOPE: &str = "outbox:replay";
#[derive(Debug)]
pub enum ReplayError {
    Forbidden,
    InvalidRequest,
    Persistence(PersistenceError),
}
impl From<PersistenceError> for ReplayError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}
pub async fn replay(
    repository: &impl OutboxReplay,
    principal: &ReplayPrincipal,
    intent: ReplayIntent,
) -> Result<ReplayOutcome, ReplayError> {
    if !principal.authorized() {
        return Err(ReplayError::Forbidden);
    }
    if !intent.valid() {
        return Err(ReplayError::InvalidRequest);
    }
    if intent.budget > principal.maximum_budget {
        return Err(ReplayError::Forbidden);
    }
    Ok(repository
        .replay(AuthorizedReplay {
            actor: principal.actor.clone(),
            scope: REPLAY_SCOPE.into(),
            intent,
        })
        .await?)
}
pub async fn outcome(
    repository: &impl OutboxReplay,
    principal: &ReplayPrincipal,
    id: Uuid,
) -> Result<Option<ReplayReceipt>, ReplayError> {
    if !principal.authorized() {
        return Err(ReplayError::Forbidden);
    }
    if id.is_nil() {
        return Err(ReplayError::InvalidRequest);
    }
    Ok(repository
        .replay_outcome(&principal.actor, REPLAY_SCOPE, id)
        .await?)
}
