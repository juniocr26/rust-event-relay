//! Bounded durable demo evidence around existing transport and consumer contracts.
//! Evidência demo limitada ao redor dos contratos existentes de transporte e consumo.
use super::{postgres::PostgresInventoryProcessor, rabbitmq::*};
use crate::{
    application::publisher::{EventPublisher, PublishError, PublishErrorKind},
    domain::event::EventEnvelope,
};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

pub struct DemoPublisher<P> {
    pub inner: P,
    pub pool: Option<PgPool>,
}
impl<P: EventPublisher + Sync> EventPublisher for DemoPublisher<P> {
    async fn publish(&self, event: &EventEnvelope) -> Result<(), PublishError> {
        if let Some(pool) = &self.pool {
            // Injection is durably consumed once before transport; a lost acknowledgement never sends here.
            let result=sqlx::query("UPDATE demo.faults SET injected_at=clock_timestamp() WHERE event_id=$1 AND injected_at IS NULL RETURNING run_id").bind(event.id()).fetch_optional(pool).await;
            match result {
                Ok(Some(_)) => return Err(PublishError::new(PublishErrorKind::RejectedBeforeSend)),
                Err(_) => return Err(PublishError::new(PublishErrorKind::Unavailable)),
                _ => {}
            }
        }
        let result = self.inner.publish(event).await;
        if let Some(pool) = &self.pool {
            // Optional observation must not turn a confirmed publication into a failure.
            let _ = record_publication(
                pool,
                event.id(),
                None,
                0,
                if result.is_ok() {
                    "confirmed"
                } else {
                    "unknown"
                },
            )
            .await;
        }
        result
    }
}
pub async fn record_publication(
    pool: &PgPool,
    event: Uuid,
    action: Option<Uuid>,
    ordinal: i16,
    outcome: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let run: Option<Uuid> =
        sqlx::query_scalar("SELECT run_id FROM demo.runs WHERE event_id=$1 FOR UPDATE")
            .bind(event)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(run) = run {
        sqlx::query("INSERT INTO demo.publications(observation_id,run_id,action_id,ordinal,outcome) SELECT $1,$2,$3,$4,$5 WHERE (SELECT count(*) FROM demo.publications WHERE run_id=$2)<32").bind(Uuid::now_v7()).bind(run).bind(action).bind(ordinal).bind(outcome).execute(&mut *tx).await?;
    }
    tx.commit().await
}

/// A separate pool of two bounds temporary action handlers without raising main consumer limits.
/// Pool separado de dois limita handlers temporários sem aumentar o consumidor principal.
pub async fn action_worker(pool: PgPool, mut stop: tokio::sync::watch::Receiver<bool>) {
    let _=sqlx::query("UPDATE demo.actions SET state='unknown',finished_at=clock_timestamp() WHERE state='executing'").execute(&pool).await;
    loop {
        if *stop.borrow() {
            break;
        }
        let claim:Result<Option<serde_json::Value>,_>=sqlx::query_scalar("UPDATE demo.actions SET state='executing' WHERE action_id=(SELECT action_id FROM demo.actions WHERE state='queued' ORDER BY accepted_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING to_jsonb(actions.*)").fetch_optional(&pool).await;
        if let Ok(Some(value)) = claim {
            if let Ok(action) = serde_json::from_value::<crate::application::demo::Action>(value) {
                let future =
                    tokio::time::timeout(Duration::from_secs(20), execute_action(&pool, &action));
                let state = tokio::select! {biased; _=stop.changed()=>"unknown",result=future=>if matches!(result,Ok(Ok(()))){"confirmed"}else{"unknown"}};
                let _=sqlx::query("UPDATE demo.actions SET state=$2,finished_at=clock_timestamp() WHERE action_id=$1").bind(action.action_id).bind(state).execute(&pool).await;
            }
        } else {
            tokio::select! {_=stop.changed()=>{},_=tokio::time::sleep(Duration::from_millis(500))=>{}};
        }
    }
    pool.close().await;
}
async fn execute_action(pool: &PgPool, a: &crate::application::demo::Action) -> Result<(), ()> {
    let row=sqlx::query("SELECT c.immutable_content FROM consumer.receipts c JOIN demo.runs r ON r.event_id=c.event_id WHERE r.run_id=$1 AND c.consumer_name='inventory.stock.v1'").bind(a.run_id).fetch_one(pool).await.map_err(|_|())?;
    let event: EventEnvelope =
        serde_json::from_slice(&row.get::<Vec<u8>, _>("immutable_content")).map_err(|_| ())?;
    let source = RabbitMqConfig::from_lookup(|key| match key {
        "RABBITMQ_CONNECT_TIMEOUT_MS" | "RABBITMQ_PUBLISH_TIMEOUT_MS" => Some("1000".into()),
        "RABBITMQ_QUEUE" => Some(a.queue_name.clone()),
        "RABBITMQ_ROUTING_KEY" => Some(a.queue_name.clone()),
        _ => std::env::var(key).ok(),
    })
    .map_err(|_| ())?;
    let consumer = RabbitConsumerConfig::new(
        source.clone(),
        Topology {
            exchange: "relay.quarantine".into(),
            queue: "relay.quarantine.inventory".into(),
            routing_key: "inventory".into(),
        },
        1,
    )
    .map_err(|_| ())?;
    let publisher = RabbitMqPublisher::connect(source).await.map_err(|_| ())?;
    let result = action_sessions(pool, a, &event, &publisher, consumer).await;
    let _ = publisher.close().await;
    result
}
async fn action_sessions(
    pool: &PgPool,
    a: &crate::application::demo::Action,
    event: &EventEnvelope,
    publisher: &RabbitMqPublisher,
    config: RabbitConsumerConfig,
) -> Result<(), ()> {
    let mut first = RabbitConsumerSession::connect(config.clone())
        .await
        .map_err(|_| ())?;
    let mut second = if a.copies == 3 {
        Some(
            RabbitConsumerSession::connect(config)
                .await
                .map_err(|_| ())?,
        )
    } else {
        None
    };
    let result = async {
        // Exactly the bounded calls, not a promise of exactly this many broker deliveries.
        for ordinal in 1..=a.copies {
            let published = publisher.publish(event).await;
            record_publication(
                pool,
                event.id(),
                Some(a.action_id),
                ordinal,
                if published.is_ok() {
                    "confirmed"
                } else {
                    "unknown"
                },
            )
            .await
            .map_err(|_| ())?;
            published.map_err(|_| ())?;
        }
        let barrier = tokio::sync::Barrier::new(if second.is_some() { 2 } else { 1 });
        let p1 =
            PostgresInventoryProcessor::new(pool.clone()).for_demo_action(a.action_id, "slot-1");
        if let Some(mut session) = second.take() {
            let p2 = PostgresInventoryProcessor::new(pool.clone())
                .for_demo_action(a.action_id, "slot-2");
            let (left, right) = tokio::join!(consume_copies(&mut first, &p1, 2, &barrier), async {
                let result = consume_copies(&mut session, &p2, 1, &barrier).await;
                // Prefetch can reserve the third copy here after the first ACK.
                // Release that reservation before waiting for slot-1 to finish.
                let _ = session.close().await;
                result
            });
            left?;
            right?;
        } else {
            consume_copies(&mut first, &p1, 1, &barrier).await?;
        }
        Ok(())
    }
    .await;
    if let Some(second) = second {
        let _ = second.close().await;
    }
    let _ = first.close().await;
    result
}
async fn consume_copies(
    session: &mut RabbitConsumerSession,
    processor: &PostgresInventoryProcessor,
    count: usize,
    barrier: &tokio::sync::Barrier,
) -> Result<(), ()> {
    let name = crate::domain::consumer::ConsumerName::new("inventory.stock.v1").map_err(|_| ())?;
    for index in 0..count {
        let delivery = session.next().await.map_err(|_| ())?.ok_or(())?;
        if index == 0 {
            barrier.wait().await;
        }
        let outcome = crate::application::consume::handle_delivery(
            &delivery,
            processor,
            &session.quarantine,
            &name,
            Duration::from_secs(3),
        )
        .await;
        if outcome.recover()
            || !matches!(
                outcome.primary,
                crate::application::consume::PrimaryOutcome::Processing(Ok(
                    crate::application::consumer::ProcessingOutcome::Duplicate(_)
                ))
            )
        {
            return Err(());
        }
    }
    Ok(())
}
