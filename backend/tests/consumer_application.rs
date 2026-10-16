use reliable_event_relay::{
    application::{consume::*, consumer::*, publisher::*},
    domain::{consumer::*, event::EventEnvelope},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
type Calls = Arc<Mutex<Vec<&'static str>>>;
struct Delivery {
    body: Vec<u8>,
    id: Option<String>,
    calls: Calls,
    blocked: bool,
    ack_failure: bool,
}
impl IncomingDelivery for Delivery {
    fn body(&self) -> &[u8] {
        &self.body
    }
    fn message_id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn preservation(
        &self,
        _: &ConsumerName,
        reason: QuarantineReason,
    ) -> Option<QuarantineMessage> {
        self.calls.lock().unwrap().push("build");
        if self.blocked {
            None
        } else {
            QuarantineMessage::new(
                self.body.clone(),
                serde_json::json!({}),
                self.id.clone(),
                reason,
            )
        }
    }
    async fn ack(&self) -> Result<(), PublishError> {
        self.calls.lock().unwrap().push("ack");
        if self.ack_failure {
            Err(PublishError::new(PublishErrorKind::Uncertain))
        } else {
            Ok(())
        }
    }
}
struct Processor {
    calls: Calls,
    outcome: ProcessingOutcome,
    failure: Option<ProcessingFailureKind>,
    stalled: bool,
}
impl AtomicInventoryProcessor for Processor {
    async fn process(
        &self,
        _: &ConsumerName,
        _: &InventoryEvent,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        self.calls.lock().unwrap().push("process");
        if self.stalled {
            std::future::pending::<()>().await;
        }
        self.failure
            .map_or(Ok(self.outcome), |kind| Err(ProcessingFailure::new(kind)))
    }
}
struct Quarantine {
    calls: Calls,
    failure: Option<PublishErrorKind>,
    stalled: bool,
}
impl QuarantinePublisher for Quarantine {
    async fn preserve(&self, message: &QuarantineMessage) -> Result<(), PublishError> {
        assert!(!message.body().is_empty());
        self.calls.lock().unwrap().push("preserve");
        if self.stalled {
            std::future::pending::<()>().await;
        }
        self.failure
            .map_or(Ok(()), |kind| Err(PublishError::new(kind)))
    }
}
fn fixture() -> (Delivery, Processor, Quarantine, ConsumerName) {
    let calls = Arc::new(Mutex::new(vec![]));
    let event = EventEnvelope::new(
        "inventory.units_received",
        "stock",
        "1",
        1,
        serde_json::json!({"warehouse_id":"w","sku":"s","quantity":5}),
    )
    .unwrap();
    (
        Delivery {
            body: serde_json::to_vec(&event).unwrap(),
            id: Some(event.id().to_string()),
            calls: calls.clone(),
            blocked: false,
            ack_failure: false,
        },
        Processor {
            calls: calls.clone(),
            outcome: ProcessingOutcome::Applied(EffectReceipt {
                stock_after: Stock::new(15).unwrap(),
            }),
            failure: None,
            stalled: false,
        },
        Quarantine {
            calls,
            failure: None,
            stalled: false,
        },
        ConsumerName::new("inventory.stock.v1").unwrap(),
    )
}
#[tokio::test]
async fn committed_and_duplicate_outcomes_precede_ack() {
    for duplicate in [false, true] {
        let (d, mut p, q, n) = fixture();
        if duplicate {
            p.outcome = ProcessingOutcome::Duplicate(EffectReceipt {
                stock_after: Stock::new(15).unwrap(),
            });
        }
        let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
        assert!(!out.recover());
        assert_eq!(*d.calls.lock().unwrap(), vec!["process", "ack"]);
    }
}
#[tokio::test]
async fn invalid_inputs_never_reach_persistence_and_confirm_before_ack() {
    for case in 0..5 {
        let (mut d, p, q, n) = fixture();
        let expected = match case {
            0 => {
                d.body = b"bad JSON".to_vec();
                QuarantineReason::MalformedJson
            }
            1 => {
                let mut v: serde_json::Value = serde_json::from_slice(&d.body).unwrap();
                v["event_type"] = serde_json::json!("other");
                d.body = serde_json::to_vec(&v).unwrap();
                QuarantineReason::Unsupported
            }
            2 => {
                d.id = None;
                QuarantineReason::InvalidIdentity
            }
            3 => {
                let mut v: serde_json::Value = serde_json::from_slice(&d.body).unwrap();
                v["payload"]["quantity"] = serde_json::json!(-1);
                d.body = serde_json::to_vec(&v).unwrap();
                QuarantineReason::InvalidContent
            }
            _ => {
                d.body = b"{}".to_vec();
                QuarantineReason::InvalidContent
            }
        };
        let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
        assert!(matches!(out.primary,PrimaryOutcome::Invalid(reason) if reason==expected));
        assert!(!out.recover());
        assert_eq!(*d.calls.lock().unwrap(), vec!["build", "preserve", "ack"]);
    }
}
#[tokio::test]
async fn conflict_retains_primary_and_secondary_errors() {
    for kind in [
        PublishErrorKind::Unavailable,
        PublishErrorKind::Serialization,
        PublishErrorKind::Rejected,
        PublishErrorKind::Unroutable,
        PublishErrorKind::Uncertain,
    ] {
        let (d, mut p, mut q, n) = fixture();
        p.outcome = ProcessingOutcome::IdentityConflict;
        q.failure = Some(kind);
        let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
        assert!(out.recover());
        assert!(matches!(
            out.primary,
            PrimaryOutcome::Processing(Ok(ProcessingOutcome::IdentityConflict))
        ));
        assert_eq!(out.preservation.unwrap().unwrap_err().kind(), kind);
        assert!(out.acknowledgement.is_none());
        assert_eq!(
            *d.calls.lock().unwrap(),
            vec!["process", "build", "preserve"]
        );
    }
    let (mut d, mut p, q, n) = fixture();
    p.outcome = ProcessingOutcome::IdentityConflict;
    d.ack_failure = true;
    let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
    assert!(matches!(
        out.primary,
        PrimaryOutcome::Processing(Ok(ProcessingOutcome::IdentityConflict))
    ));
    assert!(out.preservation.unwrap().is_ok());
    assert!(out.acknowledgement.unwrap().is_err());
    assert_eq!(
        *d.calls.lock().unwrap(),
        vec!["process", "build", "preserve", "ack"]
    );
}
#[tokio::test]
async fn database_errors_never_quarantine_or_ack() {
    for kind in [
        ProcessingFailureKind::PreCommit,
        ProcessingFailureKind::Unavailable,
        ProcessingFailureKind::InvalidStoredData,
        ProcessingFailureKind::CommitUncertain,
    ] {
        let (d, mut p, q, n) = fixture();
        p.failure = Some(kind);
        let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
        assert!(out.recover());
        assert_eq!(*d.calls.lock().unwrap(), vec!["process"]);
    }
}
#[tokio::test]
async fn unsupported_preservation_never_truncates_or_acks() {
    let (mut d, p, q, n) = fixture();
    d.body = vec![0; QuarantineMessage::MAX_BODY_BYTES + 1];
    d.blocked = true;
    let out = handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)).await;
    assert!(out.preservation_blocked);
    assert_eq!(*d.calls.lock().unwrap(), vec!["build"]);
}
#[tokio::test(start_paused = true)]
async fn phase_timeouts_cannot_authorize_follow_up_ack() {
    for preserving in [false, true] {
        let (mut d, mut p, mut q, n) = fixture();
        if preserving {
            d.body = b"bad".to_vec();
            q.stalled = true;
        } else {
            p.stalled = true;
        }
        let out = handle_delivery(&d, &p, &q, &n, Duration::from_millis(100)).await;
        assert!(out.recover());
        assert!(!d.calls.lock().unwrap().contains(&"ack"));
        if !preserving {
            assert!(
                matches!(out.primary,PrimaryOutcome::Processing(Err(e)) if e.kind()==ProcessingFailureKind::CommitUncertain)
            );
        }
    }
}
#[tokio::test]
async fn cancellation_has_no_compensating_follow_up() {
    for preserving in [false, true] {
        let (mut d, mut p, mut q, n) = fixture();
        if preserving {
            d.body = b"bad".to_vec();
            q.stalled = true;
        } else {
            p.stalled = true;
        }
        let mut future = Box::pin(handle_delivery(&d, &p, &q, &n, Duration::from_secs(1)));
        assert!(futures_util::poll!(&mut future).is_pending());
        // Dropping the future cannot call ACK or a second operation.
        drop(future);
        assert!(!d.calls.lock().unwrap().contains(&"ack"));
    }
}

#[tokio::test]
async fn observations_distinguish_effect_quarantine_and_ack_send_and_uncertainty() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        telemetry::metrics::{Metrics, Process},
    };
    let (mut d, mut p, q, n) = fixture();
    let metrics = Metrics::new(Process::Consumer);
    for (result, label) in [
        (
            ProcessingOutcome::Applied(EffectReceipt {
                stock_after: Stock::new(15).unwrap(),
            }),
            Obs::Applied,
        ),
        (
            ProcessingOutcome::Duplicate(EffectReceipt {
                stock_after: Stock::new(15).unwrap(),
            }),
            Obs::Duplicate,
        ),
        (ProcessingOutcome::IdentityConflict, Obs::IdentityConflict),
    ] {
        p.outcome = result;
        let outcome =
            handle_delivery_observed(&d, &p, &q, &n, Duration::from_secs(1), metrics.as_ref())
                .await;
        assert!(!outcome.recover());
        assert_eq!(metrics.count(Op::Processing, label), 1);
    }
    assert_eq!(metrics.count(Op::Quarantine, Obs::Confirmed), 1);
    assert_eq!(metrics.count(Op::Ack, Obs::Completed), 3);
    p.failure = Some(ProcessingFailureKind::CommitUncertain);
    let outcome =
        handle_delivery_observed(&d, &p, &q, &n, Duration::from_secs(1), metrics.as_ref()).await;
    assert!(outcome.recover());
    assert_eq!(metrics.count(Op::Processing, Obs::CommitUncertain), 1);
    assert_eq!(metrics.count(Op::Ack, Obs::Completed), 3);
    p.failure = None;
    p.outcome = ProcessingOutcome::Duplicate(EffectReceipt {
        stock_after: Stock::new(15).unwrap(),
    });
    d.ack_failure = true;
    handle_delivery_observed(&d, &p, &q, &n, Duration::from_secs(1), metrics.as_ref()).await;
    assert_eq!(metrics.count(Op::Ack, Obs::Uncertain), 1);
    assert_eq!(metrics.count(Op::Ack, Obs::Completed), 3);
    d.body = vec![b'x'; QuarantineMessage::MAX_BODY_BYTES + 1];
    d.blocked = true;
    let outcome =
        handle_delivery_observed(&d, &p, &q, &n, Duration::from_secs(1), metrics.as_ref()).await;
    assert!(outcome.preservation_blocked);
    assert_eq!(metrics.count(Op::Quarantine, Obs::Blocked), 1);
    assert_eq!(metrics.count(Op::Quarantine, Obs::Confirmed), 1);
    assert_eq!(metrics.count(Op::Ack, Obs::Completed), 3);
}

#[tokio::test]
async fn failed_quarantine_never_counts_confirmed_or_ack_success() {
    use reliable_event_relay::{
        application::observation::{Operation as Op, Outcome as Obs},
        telemetry::metrics::{Metrics, Process},
    };
    for (failure, label) in [
        (PublishErrorKind::Rejected, Obs::Rejected),
        (PublishErrorKind::Uncertain, Obs::Uncertain),
        (PublishErrorKind::Unavailable, Obs::Unavailable),
    ] {
        let (d, mut p, mut q, n) = fixture();
        p.outcome = ProcessingOutcome::IdentityConflict;
        q.failure = Some(failure);
        let metrics = Metrics::new(Process::Consumer);
        let outcome =
            handle_delivery_observed(&d, &p, &q, &n, Duration::from_secs(1), metrics.as_ref())
                .await;
        assert!(outcome.recover());
        assert_eq!(metrics.count(Op::Quarantine, label), 1);
        assert_eq!(metrics.count(Op::Quarantine, Obs::Confirmed), 0);
        assert_eq!(metrics.count(Op::Ack, Obs::Completed), 0);
        assert_eq!(metrics.count(Op::Processing, Obs::IdentityConflict), 1);
    }
}
