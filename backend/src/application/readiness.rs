//! Process-local admission observations; no exporter, SQL or broker dependencies.
use std::{
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::Instant;

pub const FRESHNESS: Duration = Duration::from_secs(15);
#[derive(Clone, Copy)]
pub enum Dependency {
    Database,
    Broker,
}
impl Dependency {
    pub fn index(self) -> usize {
        match self {
            Self::Database => 0,
            Self::Broker => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Starting,
    Ready,
    DatabaseUnknown,
    DatabaseUnavailable,
    DatabaseStale,
    BrokerUnknown,
    BrokerUnavailable,
    BrokerStale,
    Stopping,
}
impl Reason {
    pub const ALL: [Self; 9] = [
        Self::Starting,
        Self::Ready,
        Self::DatabaseUnknown,
        Self::DatabaseUnavailable,
        Self::DatabaseStale,
        Self::BrokerUnknown,
        Self::BrokerUnavailable,
        Self::BrokerStale,
        Self::Stopping,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::DatabaseUnknown => "database_unknown",
            Self::DatabaseUnavailable => "database_unavailable",
            Self::DatabaseStale => "database_stale",
            Self::BrokerUnknown => "broker_unknown",
            Self::BrokerUnavailable => "broker_unavailable",
            Self::BrokerStale => "broker_stale",
            Self::Stopping => "stopping",
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Observation {
    at: Option<Instant>,
    success_at: Option<u64>,
    ok: bool,
    failures: u64,
    duration: Duration,
}
struct Inner {
    admitted: bool,
    stopping: bool,
    required: [bool; 2],
    observations: [Observation; 2],
}
pub struct Readiness(Mutex<Inner>);
#[derive(Debug)]
pub struct Snapshot {
    pub reason: Reason,
    pub last_success: [Option<u64>; 2],
    pub age: [Option<Duration>; 2],
    pub failures: [u64; 2],
    pub duration: [Duration; 2],
    pub required: [bool; 2],
}
impl Snapshot {
    pub fn ready(&self) -> bool {
        self.reason == Reason::Ready
    }
    pub fn state(&self) -> &'static str {
        match self.reason {
            Reason::Starting => "starting",
            Reason::Ready => "ready",
            Reason::Stopping => "stopping",
            _ => "not_ready",
        }
    }
}
impl Readiness {
    pub fn new(database: bool, broker: bool) -> Self {
        Self(Mutex::new(Inner {
            admitted: false,
            stopping: false,
            required: [database, broker],
            observations: [Observation::default(); 2],
        }))
    }
    pub fn require_database(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).required[0] = true;
    }
    pub fn admit(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).admitted = true;
    }
    pub fn stop(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).stopping = true;
    }
    pub fn observe(&self, dependency: Dependency, ok: bool, duration: Duration) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let o = &mut state.observations[dependency.index()];
        o.at = Some(Instant::now());
        o.ok = ok;
        o.duration = duration;
        if ok {
            o.success_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|v| v.as_secs());
        } else {
            o.failures = o.failures.saturating_add(1);
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        let s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let age = s
            .observations
            .map(|o| o.at.map(|at| now.saturating_duration_since(at)));
        let mut reason = if s.stopping {
            Reason::Stopping
        } else if !s.admitted {
            Reason::Starting
        } else {
            Reason::Ready
        };
        if reason == Reason::Ready {
            for i in 0..2 {
                if !s.required[i] {
                    continue;
                }
                reason = match age[i] {
                    None => [Reason::DatabaseUnknown, Reason::BrokerUnknown][i],
                    Some(a) if a > FRESHNESS => [Reason::DatabaseStale, Reason::BrokerStale][i],
                    Some(_) if !s.observations[i].ok => {
                        [Reason::DatabaseUnavailable, Reason::BrokerUnavailable][i]
                    }
                    _ => Reason::Ready,
                };
                if reason != Reason::Ready {
                    break;
                }
            }
        }
        Snapshot {
            reason,
            age,
            required: s.required,
            last_success: s.observations.map(|o| o.success_at),
            failures: s.observations.map(|o| o.failures),
            duration: s.observations.map(|o| o.duration),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn transitions_degradation_recovery_freshness_and_sticky_shutdown() {
        let s = Readiness::new(true, true);
        assert_eq!(s.snapshot().reason, Reason::Starting);
        s.admit();
        assert_eq!(s.snapshot().reason, Reason::DatabaseUnknown);
        s.observe(Dependency::Database, true, Duration::ZERO);
        assert_eq!(s.snapshot().reason, Reason::BrokerUnknown);
        s.observe(Dependency::Broker, true, Duration::ZERO);
        assert!(s.snapshot().ready());
        s.observe(Dependency::Database, false, Duration::ZERO);
        assert_eq!(s.snapshot().reason, Reason::DatabaseUnavailable);
        assert!(s.snapshot().last_success[0].is_some());
        assert_eq!(s.snapshot().failures[0], 1);
        s.observe(Dependency::Database, true, Duration::ZERO);
        assert!(s.snapshot().ready());
        s.observe(Dependency::Broker, false, Duration::ZERO);
        assert_eq!(s.snapshot().reason, Reason::BrokerUnavailable);
        s.observe(Dependency::Broker, true, Duration::ZERO);
        tokio::time::advance(FRESHNESS).await;
        assert!(s.snapshot().ready());
        tokio::time::advance(Duration::from_nanos(1)).await;
        assert_eq!(s.snapshot().reason, Reason::DatabaseStale);
        s.observe(Dependency::Database, true, Duration::ZERO);
        assert_eq!(s.snapshot().reason, Reason::BrokerStale);
        s.stop();
        s.admit();
        s.observe(Dependency::Broker, true, Duration::ZERO);
        assert_eq!(s.snapshot().reason, Reason::Stopping);
    }
    #[test]
    fn optional_database_is_required_only_when_enabled() {
        let s = Readiness::new(false, false);
        s.admit();
        assert!(s.snapshot().ready());
        s.require_database();
        assert_eq!(s.snapshot().reason, Reason::DatabaseUnknown);
        let labels: std::collections::HashSet<_> =
            Reason::ALL.map(Reason::label).into_iter().collect();
        assert_eq!(labels.len(), 9);
    }
}
