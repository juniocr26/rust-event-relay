use reliable_event_relay::{
    application::maintenance::{MaintenanceOutcome, maintain_once},
    domain::retry::TerminalReason,
    persistence::*,
};
use std::sync::Mutex;
use uuid::Uuid;
struct Fake {
    next: Mutex<Option<Result<Option<DiscoveryObservation>, PersistenceError>>>,
    calls: Mutex<usize>,
    error: Option<PersistenceErrorKind>,
}
impl OutboxCandidateDiscovery for Fake {
    async fn discover_candidate(
        &self,
        _: DiscoveryCursor,
    ) -> Result<Option<DiscoveryObservation>, PersistenceError> {
        self.next.lock().unwrap().take().unwrap_or(Ok(None))
    }
}
impl OutboxCandidateIsolator for Fake {
    async fn isolate_candidate(
        &self,
        _: MaintenanceCandidate,
    ) -> Result<RetryTransition<PreservedFailure>, PersistenceError> {
        *self.calls.lock().unwrap() += 1;
        match self.error {
            Some(kind) => Err(PersistenceError::new(kind)),
            None => Ok(RetryTransition::Conflict),
        }
    }
}
#[tokio::test]
async fn consumes_observation_before_conflict_or_uncertainty_and_requires_fresh_discovery() {
    for error in [
        None,
        Some(PersistenceErrorKind::OperationFailed),
        Some(PersistenceErrorKind::CommitUncertain),
    ] {
        let expected = ExhaustedEvent::new(Uuid::from_u128(1), 1, 3).unwrap();
        let evidence = CandidateEvidence::Exhausted(TerminalReason::AcquisitionBudgetExhausted);
        let cursor = DiscoveryCursor {
            after: Some(expected.id()),
            through: Some(Uuid::from_u128(2)),
        };
        let repo = Fake {
            next: Mutex::new(Some(Ok(Some(DiscoveryObservation {
                cursor,
                evidence: evidence.clone(),
                candidate: Some(MaintenanceCandidate { expected, evidence }),
            })))),
            calls: Mutex::new(0),
            error,
        };
        let mut progress = DiscoveryCursor::default();
        assert!(matches!(
            maintain_once(&repo, &mut progress).await.unwrap(),
            MaintenanceOutcome::Isolation(_)
        ));
        assert_eq!(progress.after, cursor.after);
        assert!(matches!(
            maintain_once(&repo, &mut progress).await.unwrap(),
            MaintenanceOutcome::EndOfTraversal
        ));
        assert!(progress.after.is_none());
        assert_eq!(*repo.calls.lock().unwrap(), 1);
    }
}
#[tokio::test]
async fn discovery_errors_never_authorize_isolation_or_erase_progress() {
    for kind in [
        PersistenceErrorKind::Unavailable,
        PersistenceErrorKind::InvalidStoredData,
        PersistenceErrorKind::OperationFailed,
    ] {
        let repo = Fake {
            next: Mutex::new(Some(Err(PersistenceError::new(kind)))),
            calls: Mutex::new(0),
            error: None,
        };
        let mut cursor = DiscoveryCursor {
            after: Some(Uuid::from_u128(1)),
            through: Some(Uuid::from_u128(3)),
        };
        assert_eq!(
            maintain_once(&repo, &mut cursor).await.unwrap_err().kind(),
            kind
        );
        assert_eq!(cursor.after, Some(Uuid::from_u128(1)));
        assert_eq!(*repo.calls.lock().unwrap(), 0);
    }
}
#[tokio::test]
async fn unsafe_and_live_observations_advance_without_mutation() {
    for evidence in [
        CandidateEvidence::Valid,
        CandidateEvidence::LiveOwner,
        CandidateEvidence::NotDue,
        CandidateEvidence::RepairRequired,
        CandidateEvidence::UnsupportedRepresentation,
    ] {
        let cursor = DiscoveryCursor {
            after: Some(Uuid::from_u128(1)),
            through: Some(Uuid::from_u128(3)),
        };
        let repo = Fake {
            next: Mutex::new(Some(Ok(Some(DiscoveryObservation {
                cursor,
                evidence,
                candidate: None,
            })))),
            calls: Mutex::new(0),
            error: None,
        };
        let mut progress = DiscoveryCursor::default();
        assert!(matches!(
            maintain_once(&repo, &mut progress).await.unwrap(),
            MaintenanceOutcome::Observed { .. }
        ));
        assert_eq!(progress.after, cursor.after);
        assert_eq!(*repo.calls.lock().unwrap(), 0);
    }
}
