//! One producer commit and statement-snapshot evidence, isolated from delivery ownership.
//! Commit único do produtor e evidência por snapshot de consulta.
use crate::application::demo::{Error, Evidence, Intent, Repository};
use sqlx::{PgPool, Row};
use uuid::Uuid;
fn operation_error(error: sqlx::Error) -> Error {
    if error.as_database_error().is_some() {
        Error::Rejected
    } else {
        Error::Unavailable
    }
}
fn commit_error(error: sqlx::Error) -> Error {
    if let Some(code) = error.as_database_error().and_then(|e| e.code())
        && (code.starts_with("23") || code.starts_with("40") || code == "P0001")
    {
        return Error::Rejected;
    }
    Error::CommitUnknown
}
#[derive(Clone)]
pub struct DemoRepository {
    pool: PgPool,
}
impl DemoRepository {
    pub async fn eligible_runs(&self) -> Result<Vec<serde_json::Value>, Error> {
        sqlx::query_scalar("SELECT jsonb_build_object('run_id',r.run_id,'request_id',r.request_id,'event_id',r.event_id) FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id JOIN consumer.receipts c ON c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1' WHERE o.status='processed' AND c.completed_at IS NOT NULL AND c.stock_after=15 ORDER BY r.accepted_at DESC LIMIT 100").fetch_all(&self.pool).await.map_err(|_|Error::Unavailable)
    }
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
const EVIDENCE: &str = "SELECT jsonb_build_object(
'run_id',r.run_id,'request_id',r.request_id,'event_id',r.event_id,
'warehouse_id',r.warehouse_id,'sku',r.sku,'scenario',r.scenario,
'initial_stock',r.initial_stock,'quantity',r.quantity,'accepted_at',r.accepted_at,
'fact_id',f.fact_id,
'action',(SELECT to_jsonb(a) FROM demo.actions a WHERE a.run_id=r.run_id ORDER BY accepted_at DESC,action_id DESC LIMIT 1),
'actions',COALESCE((SELECT jsonb_agg(to_jsonb(a) ORDER BY accepted_at,action_id) FROM demo.actions a WHERE a.run_id=r.run_id),'[]'::jsonb),
'effect_receipts',(SELECT count(*) FROM consumer.receipts c WHERE c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1'),
'observations',COALESCE((SELECT jsonb_agg(jsonb_build_object('observation_id',v.observation_id,'action_id',v.action_id,'context',v.context,'outcome',v.outcome,'observed_at',v.observed_at) ORDER BY v.observed_at,v.observation_id) FROM demo.consumer_observations v WHERE v.run_id=r.run_id),'[]'::jsonb),
'publications',COALESCE((SELECT jsonb_agg(jsonb_build_object('action_id',v.action_id,'ordinal',v.ordinal,'outcome',v.outcome,'observed_at',v.observed_at) ORDER BY v.observed_at) FROM demo.publications v WHERE v.run_id=r.run_id),'[]'::jsonb),
'fault',(SELECT jsonb_build_object('condition',condition,'injected_at',injected_at,'retry_available_at',retry_available_at,'retry_revision',retry_revision) FROM demo.faults v WHERE v.run_id=r.run_id),
'outbox',jsonb_build_object('status',o.status,'created_at',o.created_at,'processed_at',o.processed_at,'attempt_count',o.attempt_count,'available_at',o.available_at),
'receipt',CASE WHEN c.completed_at IS NULL THEN NULL ELSE jsonb_build_object('consumer_name',c.consumer_name,'event_id',c.event_id,'quantity',c.quantity,'stock_before',c.stock_before,'stock_after',c.stock_after,'completed_at',c.completed_at) END,
'stock',s.units,'observed_at',clock_timestamp()) AS evidence
FROM demo.runs r JOIN demo.goods_receipts f USING(run_id)
JOIN relay.outbox_events o ON o.id=r.event_id
LEFT JOIN consumer.receipts c ON c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1'
LEFT JOIN consumer.stock s ON s.warehouse_id=r.warehouse_id AND s.sku=r.sku
WHERE r.request_id=$1";
impl Repository for DemoRepository {
    async fn eligible_runs(&self) -> Result<Vec<serde_json::Value>, Error> {
        DemoRepository::eligible_runs(self).await
    }
    async fn evidence(&self, id: Uuid) -> Result<Option<Evidence>, Error> {
        let row = sqlx::query(EVIDENCE)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| Error::Unavailable)?;
        row.map(|r| {
            let v: serde_json::Value = r.try_get("evidence").map_err(|_| Error::Unavailable)?;
            serde_json::from_value(v).map_err(|_| Error::Unavailable)
        })
        .transpose()
    }
    async fn submit(&self, intent: Intent) -> Result<Evidence, Error> {
        if !intent.supported() {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(|_| Error::Unavailable)?;
        // Global bounded demo admission, not a lock on relay/consumer work.
        sqlx::query("SELECT pg_advisory_xact_lock(610017)")
            .execute(&mut *tx)
            .await
            .map_err(|_| Error::Unavailable)?;
        let prior = sqlx::query(
            "SELECT scenario,initial_stock,quantity FROM demo.runs WHERE request_id=$1",
        )
        .bind(intent.request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| Error::Unavailable)?;
        if let Some(row) = prior {
            if row.get::<String, _>("scenario") != intent.scenario
                || row.get::<i64, _>("initial_stock") != intent.initial_stock
                || row.get::<i64, _>("quantity") != intent.quantity
            {
                return Err(Error::Conflict);
            }
            tx.rollback().await.map_err(|_| Error::Unavailable)?;
        } else {
            let (retained, active): (i64,i64) = sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE o.status='pending' AND NOT EXISTS(SELECT 1 FROM consumer.receipts c WHERE c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1' AND c.completed_at IS NOT NULL)) FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id").fetch_one(&mut *tx).await.map_err(|_|Error::Unavailable)?;
            if retained >= 100 || active >= 4 {
                return Err(Error::Capacity);
            }
            let run = Uuid::now_v7();
            let event = Uuid::now_v7();
            let fact = Uuid::now_v7();
            let warehouse = format!("demo-{run}");
            let sku = format!("SKU-{run}");
            sqlx::query("INSERT INTO consumer.stock(warehouse_id,sku,units) VALUES($1,$2,10)")
                .bind(&warehouse)
                .bind(&sku)
                .execute(&mut *tx)
                .await
                .map_err(operation_error)?;
            sqlx::query("INSERT INTO demo.runs(run_id,request_id,event_id,warehouse_id,sku,scenario,initial_stock,quantity) VALUES($1,$2,$3,$4,$5,$6,10,5)").bind(run).bind(intent.request_id).bind(event).bind(&warehouse).bind(&sku).bind(&intent.scenario).execute(&mut *tx).await.map_err(operation_error)?;
            sqlx::query("INSERT INTO demo.goods_receipts(fact_id,run_id,warehouse_id,sku,quantity) VALUES($1,$2,$3,$4,5)").bind(fact).bind(run).bind(&warehouse).bind(&sku).execute(&mut *tx).await.map_err(operation_error)?;
            // PostgreSQL supplies the canonical microsecond timestamp. Only the relay restores/publishes it.
            sqlx::query("INSERT INTO relay.outbox_events(id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,correlation_id,payload) VALUES($1,'inventory.units_received','stock',$2,1,clock_timestamp(),$3,$4)").bind(event).bind(&sku).bind(run).bind(serde_json::json!({"warehouse_id":warehouse,"sku":sku,"quantity":5})).execute(&mut *tx).await.map_err(operation_error)?;
            if intent.scenario == "recovery" {
                sqlx::query("INSERT INTO demo.faults(run_id,event_id,condition) VALUES($1,$2,'reject_once_before_send')").bind(run).bind(event).execute(&mut *tx).await.map_err(operation_error)?;
            }
            tx.commit().await.map_err(commit_error)?;
        }
        // A failed post-commit read is unavailable, never a rollback claim.
        self.evidence(intent.request_id)
            .await?
            .ok_or(Error::Unavailable)
    }
}

impl crate::application::demo::Actions for DemoRepository {
    async fn submit_action(
        &self,
        intent: crate::application::demo::ActionIntent,
    ) -> Result<crate::application::demo::Action, Error> {
        use crate::application::demo::Action;
        let mut tx = self.pool.begin().await.map_err(operation_error)?;
        sqlx::query("SELECT pg_advisory_xact_lock(610017)")
            .execute(&mut *tx)
            .await
            .map_err(operation_error)?;
        let prior: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT to_jsonb(a) FROM demo.actions a WHERE request_id=$1")
                .bind(intent.request_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(operation_error)?;
        if let Some(prior) = prior {
            let action: Action = serde_json::from_value(prior).map_err(|_| Error::Unavailable)?;
            if action.request_id != intent.request_id
                || action.run_id != intent.run_id
                || action.copies != intent.copies
            {
                return Err(Error::Conflict);
            }
            tx.rollback().await.map_err(operation_error)?;
            return Ok(action);
        }
        if intent.request_id.is_nil() || !matches!(intent.copies, 1 | 3) {
            return Err(Error::Invalid);
        }
        let eligible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id JOIN consumer.receipts c ON c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1' WHERE r.run_id=$1 AND o.status='processed' AND c.stock_after=15 AND c.completed_at IS NOT NULL AND ($2=1 OR r.scenario='concurrent'))").bind(intent.run_id).bind(intent.copies).fetch_one(&mut *tx).await.map_err(operation_error)?;
        if !eligible {
            return Err(Error::Invalid);
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM demo.actions WHERE state IN ('queued','executing')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(operation_error)?;
        let (retained, per_run): (i64, i64) =
            sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE run_id=$1) FROM demo.actions")
                .bind(intent.run_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(operation_error)?;
        if count >= 4 || retained >= 100 || per_run >= 3 {
            return Err(Error::Capacity);
        }
        let id = Uuid::now_v7();
        let value:serde_json::Value=sqlx::query_scalar("INSERT INTO demo.actions(action_id,request_id,run_id,copies,queue_name) VALUES($1,$2,$3,$4,$5) RETURNING to_jsonb(actions.*)").bind(id).bind(intent.request_id).bind(intent.run_id).bind(intent.copies).bind(format!("relay.demo.{id}")).fetch_one(&mut *tx).await.map_err(operation_error)?;
        tx.commit().await.map_err(commit_error)?;
        serde_json::from_value(value).map_err(|_| Error::Unavailable)
    }
}
