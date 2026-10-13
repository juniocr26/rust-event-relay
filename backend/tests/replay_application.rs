use reliable_event_relay::{
    application::replay::{self, ReplayError, ReplayPrincipal},
    persistence::*,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;
struct Never(AtomicUsize);
impl OutboxReplay for Never {
    async fn replay(&self, _: AuthorizedReplay) -> Result<ReplayOutcome, PersistenceError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        unreachable!()
    }
    async fn replay_outcome(
        &self,
        _: &str,
        _: &str,
        _: Uuid,
    ) -> Result<Option<ReplayReceipt>, PersistenceError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        unreachable!()
    }
}
fn intent() -> ReplayIntent {
    ReplayIntent {
        request_id: Uuid::now_v7(),
        event_id: Uuid::now_v7(),
        expected_revision: 1,
        expected_cycle: 1,
        reason: "topology corrected".into(),
        budget: 10,
        delay_ms: 0,
        ticket: None,
    }
}
#[tokio::test]
async fn deny_every_path_before_io_and_validate_budget() {
    let repo = Never(AtomicUsize::new(0));
    let denied = ReplayPrincipal::authenticated("observer".into(), false, 10);
    for _ in 0..2 {
        assert!(matches!(
            replay::replay(&repo, &denied, intent()).await,
            Err(ReplayError::Forbidden)
        ));
    }
    assert!(matches!(
        replay::outcome(&repo, &denied, Uuid::now_v7()).await,
        Err(ReplayError::Forbidden)
    ));
    let limited = ReplayPrincipal::authenticated("operator".into(), true, 1);
    assert!(matches!(
        replay::replay(&repo, &limited, intent()).await,
        Err(ReplayError::Forbidden)
    ));
    let mut bad = intent();
    bad.reason = "\nsecret".into();
    assert!(matches!(
        replay::replay(&repo, &limited, bad).await,
        Err(ReplayError::InvalidRequest)
    ));
    assert_eq!(repo.0.load(Ordering::SeqCst), 0);
}
#[test]
fn structural_boundaries() {
    let mut i = intent();
    assert!(i.valid());
    i.reason = " ".into();
    assert!(!i.valid());
    i.reason = "x".repeat(257);
    assert!(!i.valid());
    i.reason = "ok".into();
    i.delay_ms = 86_400_001;
    assert!(!i.valid());
    i.delay_ms = 0;
    i.budget = 0;
    assert!(!i.valid());
    i.budget = 100;
    assert!(i.valid());
    i.request_id = Uuid::nil();
    assert!(!i.valid());
}
