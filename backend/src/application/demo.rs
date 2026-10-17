//! Fixed local demonstration contracts; no broker administration or arbitrary input.
//! Contratos fixos locais; sem administração do broker ou entrada arbitrária.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub request_id: Uuid,
    pub scenario: String,
    pub initial_stock: i64,
    pub quantity: i64,
}
impl Intent {
    pub fn supported(&self) -> bool {
        !self.request_id.is_nil()
            && matches!(
                self.scenario.as_str(),
                "normal" | "dedupe" | "concurrent" | "recovery"
            )
            && self.initial_stock == 10
            && self.quantity == 5
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Error {
    Invalid,
    Conflict,
    Capacity,
    Unavailable,
    Rejected,
    CommitUnknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvidence {
    pub status: String,
    pub created_at: String,
    pub processed_at: Option<String>,
    pub attempt_count: i32,
    pub available_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEvidence {
    pub consumer_name: String,
    pub event_id: Uuid,
    pub quantity: i64,
    pub stock_before: i64,
    pub stock_after: i64,
    pub completed_at: String,
}
/// Narrow evidence DTO, never arbitrary event payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub effect_receipts: i64,
    #[serde(default)]
    pub observations: Vec<ConsumerObservation>,
    #[serde(default)]
    pub publications: Vec<PublicationObservation>,
    #[serde(default)]
    pub fault: Option<FaultEvidence>,
    pub run_id: Uuid,
    pub request_id: Uuid,
    pub event_id: Uuid,
    pub warehouse_id: String,
    pub sku: String,
    pub scenario: String,
    pub initial_stock: i64,
    pub quantity: i64,
    pub accepted_at: String,
    pub fact_id: Uuid,
    pub outbox: OutboxEvidence,
    pub receipt: Option<ReceiptEvidence>,
    pub stock: Option<i64>,
    pub observed_at: String,
}
pub trait Repository: Send + Sync {
    fn eligible_runs(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<serde_json::Value>, Error>> + Send {
        async { Ok(vec![]) }
    }
    fn submit(
        &self,
        intent: Intent,
    ) -> impl std::future::Future<Output = Result<Evidence, Error>> + Send;
    fn evidence(
        &self,
        request_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<Evidence>, Error>> + Send;
}
pub async fn submit(repo: &impl Repository, intent: Intent) -> Result<Evidence, Error> {
    // Reconciliation precedes supported-input validation so changed intent conflicts.
    if let Some(prior) = repo.evidence(intent.request_id).await? {
        if prior.scenario != intent.scenario
            || prior.initial_stock != intent.initial_stock
            || prior.quantity != intent.quantity
        {
            return Err(Error::Conflict);
        }
        return Ok(prior);
    }
    if !intent.supported() {
        return Err(Error::Invalid);
    }
    repo.submit(intent).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerObservation {
    pub observation_id: Uuid,
    pub action_id: Option<Uuid>,
    pub context: String,
    pub outcome: String,
    pub observed_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationObservation {
    pub action_id: Option<Uuid>,
    pub ordinal: i16,
    pub outcome: String,
    pub observed_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultEvidence {
    pub condition: String,
    pub injected_at: Option<String>,
    pub retry_available_at: Option<String>,
    pub retry_revision: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_id: Uuid,
    pub request_id: Uuid,
    pub run_id: Uuid,
    pub copies: i16,
    pub state: String,
    pub accepted_at: String,
    pub finished_at: Option<String>,
    pub queue_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionIntent {
    pub request_id: Uuid,
    pub run_id: Uuid,
    pub copies: i16,
}
pub trait Actions: Send + Sync {
    fn submit_action(
        &self,
        intent: ActionIntent,
    ) -> impl std::future::Future<Output = Result<Action, Error>> + Send;
}
