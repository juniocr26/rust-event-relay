//! Sequential admission, explicit publisher replacement and finite shutdown.
use super::{
    delivery::{DeliveryOutcome, deliver_once},
    publisher::{EventPublisher, PublishError, PublishErrorKind},
};
use crate::{
    config::WorkerConfig,
    persistence::{
        AcquireRequest, OutboxAcquirer, OutboxCompleter, OutboxReleaser, TransitionOutcome,
    },
};
use std::{future::Future, time::Duration};
use tokio::time::{Instant, sleep, timeout_at};

/// Infrastructure construction stays at composition; no implicit recovery.
pub trait PublisherLifecycle {
    type Publisher: EventPublisher + Sync;
    fn connect(&self) -> impl Future<Output = Result<Self::Publisher, PublishError>> + Send;
    fn usable(&self, publisher: &Self::Publisher) -> bool;
    fn close(
        &self,
        publisher: Self::Publisher,
    ) -> impl Future<Output = Result<(), PublishError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Idle,
    Continue,
    Pace,
    Replace,
}

/// All secondary transition results are retained in the logged outcome.
pub fn decision(outcome: &DeliveryOutcome) -> Decision {
    match outcome {
        DeliveryOutcome::NoEvent => Decision::Idle,
        DeliveryOutcome::Published {
            completion: Ok(TransitionOutcome::Applied),
            ..
        } => Decision::Continue,
        DeliveryOutcome::PublicationFailed { error, .. }
            if matches!(
                error.kind(),
                PublishErrorKind::Unavailable | PublishErrorKind::Uncertain
            ) =>
        {
            Decision::Replace
        }
        DeliveryOutcome::AcquisitionFailed { .. }
        | DeliveryOutcome::Published { .. }
        | DeliveryOutcome::PublicationFailed { .. } => Decision::Pace,
    }
}

/// Await a single owned operation. On signal drain it, then cancel by dropping.
/// Returns the overall cleanup deadline even when an outcome is suppressed.
pub async fn drain_on_shutdown<T>(
    operation: impl Future<Output = T>,
    shutdown: impl Future<Output = ()>,
    drain: Duration,
    budget: Duration,
) -> (Option<T>, Option<Instant>) {
    tokio::pin!(operation);
    tokio::select! {
        biased;
        () = shutdown => {
            let now = Instant::now();
            let deadline = now + budget;
            let result = timeout_at(now + drain, &mut operation).await.ok();
            if result.is_none() { tracing::warn!("operation cancelled; durable effects may have occurred; no compensating release"); }
            (result, Some(deadline))
        }
        result = &mut operation => (Some(result), None),
    }
}

async fn wait(delay: Duration, shutdown: &mut (impl Future<Output = ()> + Unpin)) -> bool {
    tokio::select! { biased; () = shutdown => true, () = sleep(delay) => false }
}

/// One operation future at a time, no spawned tasks, no acquisition without publisher.
/// Returns an absolute deadline so pool closure shares the same shutdown budget.
pub async fn run<R, L>(
    repository: &R,
    lifecycle: &L,
    config: &WorkerConfig,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
) -> Instant
where
    R: OutboxAcquirer + OutboxCompleter + OutboxReleaser,
    L: PublisherLifecycle,
{
    let mut publisher = None;
    let deadline = 'worker: loop {
        // Poll signal before constructing/admitting any operation.
        tokio::select! { biased; () = &mut *shutdown => break Instant::now() + config.shutdown, _ = std::future::ready(()) => {} }
        if publisher.is_none() {
            tracing::info!("publisher connecting");
            let (result, stop) = drain_on_shutdown(
                lifecycle.connect(),
                &mut *shutdown,
                config.drain,
                config.shutdown,
            )
            .await;
            if let Some(Ok(p)) = result {
                publisher = Some(p);
                tracing::info!("publisher connected");
            }
            if let Some(deadline) = stop {
                break deadline;
            }
            if publisher.is_none() {
                tracing::warn!(
                    category = "Unavailable",
                    "publisher connection failed (source redacted)"
                );
                if wait(config.reconnect, shutdown).await {
                    break Instant::now() + config.shutdown;
                }
                continue;
            }
        }
        if publisher.as_ref().is_some_and(|p| !lifecycle.usable(p)) {
            let old = publisher.take().expect("present publisher");
            let (closed, stop) = drain_on_shutdown(
                lifecycle.close(old),
                &mut *shutdown,
                config.drain,
                config.shutdown,
            )
            .await;
            if let Some(Err(error)) = closed {
                tracing::warn!(error = %error, "publisher replacement close failed");
            }
            if let Some(deadline) = stop {
                break deadline;
            }
            if wait(config.reconnect, shutdown).await {
                break Instant::now() + config.shutdown;
            }
            continue;
        }
        let request = AcquireRequest::new(config.lease);
        let (outcome, stop) = drain_on_shutdown(
            deliver_once(
                repository,
                publisher.as_ref().expect("connected publisher"),
                request,
            ),
            &mut *shutdown,
            config.drain,
            config.shutdown,
        )
        .await;
        let action = outcome.as_ref().map(decision);
        if let Some(outcome) = outcome {
            tracing::info!(outcome = ?outcome, action = ?action, "delivery outcome");
        }
        if let Some(deadline) = stop {
            break deadline;
        }
        let delay = match action.expect("completed operation") {
            Decision::Continue => continue,
            Decision::Idle => config.idle,
            Decision::Pace => config.failure,
            Decision::Replace => {
                let old = publisher.take().expect("connected publisher");
                let (closed, stop) = drain_on_shutdown(
                    lifecycle.close(old),
                    &mut *shutdown,
                    config.drain,
                    config.shutdown,
                )
                .await;
                if let Some(Err(error)) = closed {
                    tracing::warn!(error = %error, "publisher replacement close failed");
                }
                if let Some(deadline) = stop {
                    break 'worker deadline;
                }
                config.reconnect
            }
        };
        if wait(delay, shutdown).await {
            break Instant::now() + config.shutdown;
        }
    };
    if let Some(publisher) = publisher {
        match timeout_at(deadline, lifecycle.close(publisher)).await {
            Ok(Ok(())) => tracing::info!("publisher closed"),
            _ => tracing::warn!("publisher cleanup failed or exhausted shutdown budget"),
        }
    }
    deadline
}
