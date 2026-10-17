//! Single-flight, read-only database observations, independent of HTTP requests.
use crate::application::readiness::{Dependency, Readiness};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::oneshot,
    task::JoinHandle,
    time::{Instant, timeout},
};

pub const INTERVAL: Duration = Duration::from_secs(5);
pub const DEADLINE: Duration = Duration::from_secs(2);
/// Columns used by each role. LIMIT 0 validates relation/column privileges without scanning rows.
pub const RELAY_SCHEMA: &str = "SELECT id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at, correlation_id, causation_id, payload, status, attempt_count, available_at, created_at, processed_at, ownership_token, acquired_at, expires_at, retry_cycle, cycle_baseline, cycle_maximum, state_revision, diagnostic_code, diagnostic_at, failure_evidence, terminal_at, terminal_reason FROM relay.outbox_events LIMIT 0";
pub const CONSUMER_SCHEMA: &[&str] = &[
    "SELECT warehouse_id,sku,units FROM consumer.stock LIMIT 0",
    "SELECT consumer_name,event_id,representation_version,immutable_content,warehouse_id,sku,quantity,stock_before,stock_after,completed_at FROM consumer.receipts LIMIT 0",
];
pub const REPLAY_SCHEMA: &str = "SELECT request_id,actor,scope,event_id,intent,receipt,prior_terminal,recorded_at FROM relay.replay_requests LIMIT 0";
/// One additional pool connection per role requiring PostgreSQL; no waiting caller queue.
pub struct DatabaseObservation {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    pool: PgPool,
}
impl DatabaseObservation {
    pub fn start(
        options: PgConnectOptions,
        queries: &'static [&'static str],
        state: Arc<Readiness>,
    ) -> Self {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(DEADLINE)
            .connect_lazy_with(options.options([
                ("statement_timeout", "1000"),
                ("lock_timeout", "500"),
                ("default_transaction_read_only", "on"),
            ]));
        let task_pool = pool.clone();
        let (tx, mut rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                let started = Instant::now();
                let ok = tokio::select! { biased;
                    _=&mut rx=>break,
                    result=timeout(DEADLINE, async {
                        // One connection held for this observation; sequential statements.
                        let mut connection=task_pool.acquire().await?;
                        for query in queries { sqlx::query(query).execute(&mut *connection).await?; }
                        Ok::<_,sqlx::Error>(())
                    })=>matches!(result,Ok(Ok(()))),
                };
                state.observe(Dependency::Database, ok, started.elapsed());
                // Delay from completion, no catch-up bursts or overlapping samples.
                tokio::select! { biased; _=&mut rx=>break, ()=tokio::time::sleep(INTERVAL)=>{} }
            }
        });
        Self {
            stop: Some(tx),
            task,
            pool,
        }
    }
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if timeout(Duration::from_secs(1), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            let _ = (&mut self.task).await;
        }
        let _ = timeout(Duration::from_secs(1), self.pool.close()).await;
    }
}
impl Drop for DatabaseObservation {
    fn drop(&mut self) {
        self.task.abort();
    }
}
