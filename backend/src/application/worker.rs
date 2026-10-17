//! Sequential admission, explicit publisher replacement and finite shutdown.
use super::observation::{self, Noop, Observer, Operation as Op, Outcome as Obs, observe};
use super::{
    delivery::{DeliveryOutcome, deliver_once_observed},
    publisher::{EventPublisher, PublishError, PublishErrorKind},
};
use crate::{
    config::WorkerConfig,
    persistence::{
        AcquireRequest, DiscoveryCursor, OutboxAcquirer, OutboxCandidateDiscovery,
        OutboxCandidateIsolator, OutboxCompleter, OutboxExhaustedMaintainer,
        OutboxFailurePreserver, OutboxReleaser, OutboxRetryScheduler, TransitionOutcome,
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
/// Every definitive event failure, including acknowledged schedules/terminal states,
/// uses failure pacing before general admission. This bounds hot topology failures
/// without waiting for one event's durable due time. Unusable/uncertain publishers
/// instead use reconnection pacing; reconnection never reconciles prior acceptance.
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
    R: OutboxAcquirer
        + OutboxCompleter
        + OutboxReleaser
        + OutboxRetryScheduler
        + OutboxFailurePreserver
        + OutboxExhaustedMaintainer
        + OutboxCandidateDiscovery
        + OutboxCandidateIsolator,
    L: PublisherLifecycle,
{
    run_observed(repository, lifecycle, config, shutdown, &Noop).await
}

pub async fn run_observed<R, L, O: Observer>(
    repository: &R,
    lifecycle: &L,
    config: &WorkerConfig,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
    observer: &O,
) -> Instant
where
    R: OutboxAcquirer
        + OutboxCompleter
        + OutboxReleaser
        + OutboxRetryScheduler
        + OutboxFailurePreserver
        + OutboxExhaustedMaintainer
        + OutboxCandidateDiscovery
        + OutboxCandidateIsolator,
    L: PublisherLifecycle,
{
    let mut publisher = None;
    let mut maintenance = config.maintenance().iter().copied();
    let mut cursor = DiscoveryCursor::default();
    let mut discovered_valid = None;
    let mut next_maintenance = Instant::now();
    let deadline = 'worker: loop {
        // Poll signal before constructing/admitting any operation.
        tokio::select! { biased; () = &mut *shutdown => break Instant::now() + config.shutdown, _ = std::future::ready(()) => {} }
        // Database-only maintenance is admitted before publisher connection.
        // At most one observation + one isolation (or one configured exact ID)
        // per second and per loop. No catch-up bursts after slow operations.
        if Instant::now() >= next_maintenance {
            let observation = super::observation::Observation::new(observer, Op::Maintenance);
            let mut discovery_error = None;
            let (result, stop) = if let Some(candidate) = maintenance.next() {
                drain_on_shutdown(
                    async {
                        super::maintenance::MaintenanceOutcome::Isolation(
                            repository.preserve_exhausted(candidate).await,
                        )
                    },
                    &mut *shutdown,
                    config.drain,
                    config.shutdown,
                )
                .await
            } else {
                let (result, stop) = drain_on_shutdown(
                    super::maintenance::maintain_once(repository, &mut cursor),
                    &mut *shutdown,
                    config.drain,
                    config.shutdown,
                )
                .await;
                if let Some(Err(error)) = &result {
                    discovery_error = Some(observation::persistence(error));
                    tracing::warn!(error = %error, "discovery failed; no isolation authorized");
                }
                (result.and_then(Result::ok), stop)
            };
            next_maintenance = Instant::now() + Duration::from_secs(1);
            if let Some(result) = &result {
                if let super::maintenance::MaintenanceOutcome::Observed {
                    cursor,
                    evidence: crate::persistence::CandidateEvidence::Valid,
                } = result
                {
                    discovered_valid = cursor.after;
                }
                tracing::info!(outcome = ?result, "database maintenance outcome");
            }
            if let Some(result) = &result {
                observation.finish(match result {
                    super::maintenance::MaintenanceOutcome::EndOfTraversal => Obs::End,
                    super::maintenance::MaintenanceOutcome::Observed { .. } => Obs::Observed,
                    super::maintenance::MaintenanceOutcome::Isolation(r) => observation::retry(r),
                });
            } else {
                observation.finish(if stop.is_some() {
                    Obs::Cancelled
                } else {
                    discovery_error.unwrap_or(Obs::Failed)
                });
            }
            if let Some(deadline) = stop {
                break deadline;
            }
        }
        if publisher.is_none() {
            observer.broker_readiness(false);
            tracing::info!("publisher connecting");
            let (result, stop) = drain_on_shutdown(
                observe(observer, Op::Connect, lifecycle.connect(), |r| {
                    if r.is_ok() {
                        Obs::Completed
                    } else {
                        observation::publication(
                            &r.as_ref()
                                .map(|_| ())
                                .map_err(|e| super::publisher::PublishError::new(e.kind())),
                        )
                    }
                }),
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
        observer.broker_readiness(publisher.as_ref().is_some_and(|p| lifecycle.usable(p)));
        if publisher.as_ref().is_some_and(|p| !lifecycle.usable(p)) {
            let old = publisher.take().expect("present publisher");
            let (closed, stop) = drain_on_shutdown(
                observe(observer, Op::Close, lifecycle.close(old), |r| {
                    if r.is_ok() {
                        Obs::Completed
                    } else {
                        observation::publication(r)
                    }
                }),
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
        let request = match discovered_valid.take() {
            Some(id) => match request.targeting(id) {
                Ok(request) => request,
                Err(_) => {
                    tracing::warn!("invalid discovered identity; no delivery admitted");
                    continue;
                }
            },
            None => request,
        };
        let (outcome, stop) = drain_on_shutdown(
            observe(
                observer,
                Op::Delivery,
                deliver_once_observed(
                    repository,
                    publisher.as_ref().expect("connected publisher"),
                    request,
                    super::retry_policy::sample_delay,
                    observer,
                ),
                |_| Obs::Completed,
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
                observer.broker_readiness(false);
                let old = publisher.take().expect("connected publisher");
                let (closed, stop) = drain_on_shutdown(
                    observe(observer, Op::Close, lifecycle.close(old), |r| {
                        if r.is_ok() {
                            Obs::Completed
                        } else {
                            observation::publication(r)
                        }
                    }),
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
        match timeout_at(
            deadline,
            observe(observer, Op::Close, lifecycle.close(publisher), |r| {
                if r.is_ok() {
                    Obs::Completed
                } else {
                    observation::publication(r)
                }
            }),
        )
        .await
        {
            Ok(Ok(())) => tracing::info!("publisher closed"),
            _ => tracing::warn!("publisher cleanup failed or exhausted shutdown budget"),
        }
    }
    deadline
}
