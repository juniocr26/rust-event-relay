//! Explicit AMQP lifecycle. No automatic Lapin recovery, per-delivery tasks or nack loop.
//! Ciclo AMQP explícito. Sem recovery automático, tarefas por entrega ou loop de nack.
use super::*;
use crate::application::observation::{
    self, Noop, Observer, Operation as Op, Outcome as Obs, observe,
};
use crate::{
    application::{
        consume::{IncomingDelivery, handle_delivery_observed},
        consumer::{
            AtomicInventoryProcessor, QuarantineMessage, QuarantinePublisher, QuarantineReason,
        },
    },
    config::ConsumerRuntimeConfig,
    domain::consumer::ConsumerName,
};
use futures_util::StreamExt;
use serde_json::json;
use std::future::Future;
use tokio::time::{Instant, sleep, timeout_at};

/// Independent source and quarantine topology; never delete/recreate on mismatch.
/// Topologias independentes; nunca remover/recriar em caso de incompatibilidade.
#[derive(Clone, Debug)]
pub struct RabbitConsumerConfig {
    source: RabbitMqConfig,
    quarantine: RabbitMqConfig,
    prefetch: u16,
}
impl RabbitConsumerConfig {
    pub fn new(
        source: RabbitMqConfig,
        quarantine: Topology,
        prefetch: u16,
    ) -> Result<Self, PublishError> {
        if prefetch == 0
            || prefetch > 32
            || source.topology.exchange == quarantine.exchange
            || source.topology.queue == quarantine.queue
        {
            return Err(PublishError::new(Kind::Unavailable));
        }
        let quarantine = RabbitMqConfig::new(
            source.uri.clone(),
            quarantine,
            source.connect_timeout,
            source.publish_timeout,
        )?;
        Ok(Self {
            source,
            quarantine,
            prefetch,
        })
    }
    pub fn from_lookup(
        get: impl Fn(&str) -> Option<String>,
        prefetch: u16,
    ) -> Result<Self, PublishError> {
        let source = RabbitMqConfig::from_lookup(|key| match key {
            "RABBITMQ_EXCHANGE" => {
                Some(get("CONSUMER_SOURCE_EXCHANGE").unwrap_or_else(|| "relay.events".into()))
            }
            "RABBITMQ_QUEUE" => {
                Some(get("CONSUMER_SOURCE_QUEUE").unwrap_or_else(|| "relay.local".into()))
            }
            "RABBITMQ_ROUTING_KEY" => {
                Some(get("CONSUMER_SOURCE_ROUTING_KEY").unwrap_or_else(|| "event".into()))
            }
            _ => get(key),
        })?;
        Self::new(
            source,
            Topology {
                exchange: get("CONSUMER_QUARANTINE_EXCHANGE")
                    .unwrap_or_else(|| "relay.quarantine".into()),
                queue: get("CONSUMER_QUARANTINE_QUEUE")
                    .unwrap_or_else(|| "relay.quarantine.inventory".into()),
                routing_key: get("CONSUMER_QUARANTINE_ROUTING_KEY")
                    .unwrap_or_else(|| "inventory".into()),
            },
            prefetch,
        )
    }
}
/// Raw preservation with confirms; the original bytes remain the publication body.
/// Preservação bruta com confirms; os bytes originais permanecem no corpo.
pub struct RabbitQuarantinePublisher(RabbitMqPublisher);
impl RabbitQuarantinePublisher {
    pub async fn connect(config: RabbitMqConfig) -> Result<Self, PublishError> {
        RabbitMqPublisher::connect(config).await.map(Self)
    }
    pub fn is_usable(&self) -> bool {
        self.0.is_usable()
    }
    pub async fn close(self) -> Result<(), PublishError> {
        self.0.close().await
    }
}
impl QuarantinePublisher for RabbitQuarantinePublisher {
    async fn preserve(&self, message: &QuarantineMessage) -> Result<(), PublishError> {
        let _gate = self
            .0
            .gate
            .try_lock()
            .map_err(|_| PublishError::new(Kind::Unavailable))?;
        let sidecar = serde_json::to_vec(message.properties())
            .map_err(|e| PublishError::with_source(Kind::Serialization, e))?;
        let mut headers = FieldTable::default();
        // Never merge original headers into trusted metadata. Entire sidecar is namespaced.
        headers.insert(
            "relay-preservation-v1".into(),
            lapin::types::AMQPValue::LongString(sidecar.into()),
        );
        let mut properties = BasicProperties::default()
            .with_delivery_mode(2)
            .with_content_type("application/octet-stream".into())
            .with_type("relay.quarantine.v1".into())
            .with_headers(headers);
        if let Some(id) = message.message_id() {
            properties = properties.with_message_id(id.into());
        }
        self.0.publish_admitted(message.body(), properties).await
    }
}
/// Carries only the original receiving channel's acker; no synthetic delivery tags.
/// Carrega apenas o acker original; sem delivery tags sintéticos.
pub struct RabbitDelivery(lapin::message::Delivery);
impl RabbitDelivery {
    pub fn from_delivery(delivery: lapin::message::Delivery) -> Self {
        Self(delivery)
    }
}
impl IncomingDelivery for RabbitDelivery {
    fn body(&self) -> &[u8] {
        &self.0.data
    }
    fn message_id(&self) -> Option<&str> {
        self.0.properties.message_id().as_ref().map(|s| s.as_str())
    }
    fn preservation(
        &self,
        consumer: &ConsumerName,
        reason: QuarantineReason,
    ) -> Option<QuarantineMessage> {
        if self.body().len() > QuarantineMessage::MAX_BODY_BYTES {
            return None;
        }
        let original = super::properties::encode_properties(&self.0.properties)?;
        let record = json!({"format_version":1,"consumer":consumer.as_str(),"source_exchange":self.0.exchange.as_str(),"source_routing_key":self.0.routing_key.as_str(),"observed_at":chrono::Utc::now(),"reason":reason,"original_properties":original});
        QuarantineMessage::new(
            self.body().to_vec(),
            record,
            self.message_id().map(str::to_owned),
            reason,
        )
    }
    async fn ack(&self) -> Result<(), PublishError> {
        match self.0.ack(BasicAckOptions { multiple: false }).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(PublishError::new(Kind::Uncertain)),
            Err(e) => Err(PublishError::with_source(Kind::Uncertain, e)),
        }
    }
}
/// One subscription with bounded broker credit and a separate confirmed publisher.
/// Uma assinatura com crédito limitado e publisher confirmado separado.
pub struct RabbitConsumerSession {
    connection: Connection,
    channel: Channel,
    consumer: lapin::Consumer,
    pub quarantine: RabbitQuarantinePublisher,
    deadline: Duration,
}
impl RabbitConsumerSession {
    pub async fn connect(config: RabbitConsumerConfig) -> Result<Self, PublishError> {
        // Establish preservation before admitting any source delivery.
        let quarantine = RabbitQuarantinePublisher::connect(config.quarantine).await?;
        let deadline = config.source.connect_timeout;
        let connection = match timeout(
            deadline,
            Connection::connect(&config.source.uri, ConnectionProperties::default()),
        )
        .await
        {
            Ok(Ok(c)) => c,
            result => {
                let _ = quarantine.close().await;
                return Err(match result {
                    Ok(Err(e)) => PublishError::with_source(Kind::Unavailable, e),
                    Err(e) => PublishError::with_source(Kind::Unavailable, e),
                    _ => unreachable!(),
                });
            }
        };
        let setup = timeout(deadline, async {
            let channel = connection.create_channel().await?;
            channel
                .exchange_declare(
                    config.source.topology.exchange.clone().into(),
                    ExchangeKind::Direct,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            let mut arguments = FieldTable::default();
            arguments.insert(
                "x-queue-type".into(),
                lapin::types::AMQPValue::LongString("classic".into()),
            );
            channel
                .queue_declare(
                    config.source.topology.queue.clone().into(),
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    arguments,
                )
                .await?;
            channel
                .queue_bind(
                    config.source.topology.queue.clone().into(),
                    config.source.topology.exchange.clone().into(),
                    config.source.topology.routing_key.into(),
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await?;
            channel
                .basic_qos(config.prefetch, BasicQosOptions { global: false })
                .await?;
            let consumer = channel
                .basic_consume(
                    config.source.topology.queue.into(),
                    "".into(),
                    BasicConsumeOptions {
                        no_ack: false,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            Ok::<_, lapin::Error>((channel, consumer))
        })
        .await;
        match setup {
            Ok(Ok((channel, consumer))) => Ok(Self {
                connection,
                channel,
                consumer,
                quarantine,
                deadline,
            }),
            result => {
                let _ = timeout(
                    deadline,
                    connection.close(200, "consumer setup failed".into()),
                )
                .await;
                let _ = quarantine.close().await;
                Err(match result {
                    Ok(Err(e)) => PublishError::with_source(Kind::Unavailable, e),
                    Err(e) => PublishError::with_source(Kind::Unavailable, e),
                    _ => unreachable!(),
                })
            }
        }
    }
    pub async fn next(&mut self) -> Result<Option<RabbitDelivery>, PublishError> {
        self.consumer
            .next()
            .await
            .transpose()
            .map(|v| v.map(RabbitDelivery))
            .map_err(|e| PublishError::with_source(Kind::Unavailable, e))
    }
    pub async fn cancel(&self) -> Result<(), PublishError> {
        timeout(
            self.deadline,
            self.channel
                .basic_cancel(self.consumer.tag(), BasicCancelOptions::default()),
        )
        .await
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?
        .map_err(|e| PublishError::with_source(Kind::Unavailable, e))
    }
    pub fn is_usable(&self) -> bool {
        self.connection.status().connected()
            && self.channel.status().connected()
            && self.quarantine.is_usable()
    }
    pub async fn close(self) -> Result<(), PublishError> {
        // Close the connection even if cancellation fails: recover all buffered unacked work.
        let cancel = self.cancel().await;
        let source = timeout(
            self.deadline,
            self.connection
                .close(200, "consumer recovery/shutdown".into()),
        )
        .await;
        let quarantine = self.quarantine.close().await;
        cancel?;
        source
            .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?
            .map_err(|e| PublishError::with_source(Kind::Unavailable, e))?;
        quarantine
    }
}
async fn close_session<O: Observer>(
    session: RabbitConsumerSession,
    deadline: Instant,
    observer: &O,
) {
    match timeout_at(
        deadline,
        observe(observer, Op::Close, session.close(), |r| {
            if r.is_ok() {
                Obs::Completed
            } else {
                observation::publication(r)
            }
        }),
    )
    .await
    {
        Ok(Ok(())) => tracing::info!("consumer session closed"),
        Ok(Err(error)) => {
            tracing::warn!(error=%error,"consumer close failed (source redacted); unacknowledged work requires recovery")
        }
        Err(_) => tracing::warn!(
            "consumer close deadline exhausted; unacknowledged work requires recovery"
        ),
    }
}
async fn pace(delay: Duration, shutdown: &mut (impl Future<Output = ()> + Unpin)) -> bool {
    tokio::select! {biased; ()=shutdown=>true, ()=sleep(delay)=>false}
}
/// One in-flight operation, explicit resubscription, interruptible pacing and finite drain.
/// Uma operação em voo, reassinatura explícita, pacing interrompível e drain finito.
pub async fn run_consumer<P: AtomicInventoryProcessor>(
    processor: &P,
    config: &ConsumerRuntimeConfig,
    rabbit: &RabbitConsumerConfig,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
) -> Instant {
    run_consumer_observed(processor, config, rabbit, shutdown, &Noop).await
}
pub async fn run_consumer_observed<P: AtomicInventoryProcessor, O: Observer>(
    processor: &P,
    config: &ConsumerRuntimeConfig,
    rabbit: &RabbitConsumerConfig,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
    observer: &O,
) -> Instant {
    loop {
        observer.broker_readiness(false);
        tokio::select! {biased; ()=&mut *shutdown=>return Instant::now()+config.shutdown,_=std::future::ready(())=>{}}
        let (connected, stop) = crate::application::worker::drain_on_shutdown(
            observe(
                observer,
                Op::Connect,
                RabbitConsumerSession::connect(rabbit.clone()),
                |r| {
                    if r.is_ok() {
                        Obs::Completed
                    } else {
                        Obs::Unavailable
                    }
                },
            ),
            &mut *shutdown,
            config.drain,
            config.shutdown,
        )
        .await;
        let mut session = match connected {
            Some(Ok(session)) => session,
            _ => {
                if let Some(deadline) = stop {
                    return deadline;
                }
                tracing::warn!(
                    "consumer setup unavailable; inspect schema/topology/type/permissions (source redacted)"
                );
                if pace(config.reconnect, shutdown).await {
                    return Instant::now() + config.shutdown;
                }
                continue;
            }
        };
        if let Some(deadline) = stop {
            close_session(session, deadline, observer).await;
            return deadline;
        }
        observer.broker_readiness(session.is_usable());
        let mut check = tokio::time::interval(Duration::from_secs(1));
        check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut stop = None;
        loop {
            let delivery = tokio::select! {biased;
                ()=&mut *shutdown=>{stop=Some(Instant::now()+config.shutdown);break;}
                _=check.tick()=>{let usable=session.is_usable(); observer.broker_readiness(usable); if !usable {break;} continue;}
                result=session.next()=>match result {Ok(Some(delivery))=>delivery,_=>break},
            };
            if !session.is_usable() {
                break;
            }
            let operation = observe(
                observer,
                Op::Delivery,
                handle_delivery_observed(
                    &delivery,
                    processor,
                    &session.quarantine,
                    &config.name,
                    config.operation,
                    observer,
                ),
                |_| Obs::Completed,
            );
            tokio::pin!(operation);
            let outcome = tokio::select! {biased;
                ()=&mut *shutdown=>{
                    let started=Instant::now();let deadline=started+config.shutdown;
                    stop=Some(deadline);
                    // Stop broker admission before draining current work. BasicCancel does not requeue.
                    let _=timeout_at(started+config.drain,session.cancel()).await;
                    timeout_at(started+config.drain,&mut operation).await.ok()
                }
                outcome=&mut operation=>Some(outcome),
            };
            let recover = outcome.as_ref().is_none_or(|o| o.recover());
            if outcome.is_none() {
                tracing::warn!(
                    "consumer operation cancelled; no ACK fabricated; durable effects may exist"
                );
            }
            if let Some(outcome) = outcome {
                if outcome.preservation_blocked {
                    tracing::error!(
                        "exact preservation exceeds supported bounds; consumer stopped for operator recovery; source retained"
                    );
                    stop = Some(Instant::now() + config.shutdown);
                }
                tracing::info!(?outcome, "consumer delivery outcome (sources redacted)");
            }
            if stop.is_some() || recover {
                break;
            }
        }
        observer.broker_readiness(false);
        let deadline = stop.unwrap_or_else(|| Instant::now() + config.shutdown);
        close_session(session, deadline, observer).await;
        if stop.is_some() {
            return deadline;
        }
        if pace(config.reconnect, shutdown).await {
            return Instant::now() + config.shutdown;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_prefetch_separation_and_configuration_bounds() {
        let source = RabbitMqConfig::new(
            "amqp://user:secret@rabbitmq/relay".into(),
            Topology::default(),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap();
        for prefetch in [0, 33] {
            assert!(
                RabbitConsumerConfig::new(
                    source.clone(),
                    Topology {
                        exchange: "relay.q".into(),
                        queue: "relay.q".into(),
                        routing_key: "q".into()
                    },
                    prefetch
                )
                .is_err()
            );
        }
        assert!(RabbitConsumerConfig::new(source, Topology::default(), 1).is_err());
        let config = ConsumerRuntimeConfig::from_lookup(|_| None).unwrap();
        assert_eq!(config.prefetch, 1);
        for key in [
            "CONSUMER_PREFETCH",
            "CONSUMER_OPERATION_MS",
            "CONSUMER_RECONNECT_MS",
            "CONSUMER_POOL_MAX",
            "CONSUMER_DRAIN_MS",
            "CONSUMER_SHUTDOWN_MS",
        ] {
            for value in ["0", "999999", "invalid"] {
                assert!(
                    ConsumerRuntimeConfig::from_lookup(|k| (k == key).then(|| value.into()))
                        .is_err()
                );
            }
        }
    }
    #[tokio::test(start_paused = true)]
    async fn reconnect_pacing_waits_and_is_interruptible() {
        let mut shutdown = std::pin::pin!(std::future::pending());
        let wait = pace(Duration::from_secs(5), &mut shutdown);
        tokio::pin!(wait);
        assert!(futures_util::poll!(&mut wait).is_pending());
        tokio::time::advance(Duration::from_secs(4)).await;
        assert!(futures_util::poll!(&mut wait).is_pending());
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(!wait.await);
        let mut shutdown = std::pin::pin!(std::future::ready(()));
        assert!(pace(Duration::from_secs(5), &mut shutdown).await);
    }
}
