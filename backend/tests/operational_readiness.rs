//! Read-only sampler faults affect only disposable databases, never shared services.
mod support;
use reliable_event_relay::{
    application::readiness::{Dependency, Readiness, Reason},
    telemetry::{
        metrics::{Exporter, Metrics, Process},
        readiness::DatabaseObservation,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

#[tokio::test]
async fn reachable_unready_ready_stopping_and_absence_do_not_poll_dependencies() {
    let metrics = Metrics::new(Process::Consumer);
    let exporter = Exporter::start("127.0.0.1:0".parse().unwrap(), metrics.clone())
        .await
        .unwrap();
    let address = exporter.address();
    for (code, reason) in [
        (503, "starting"),
        (503, "database_unknown"),
        (503, "database_unavailable"),
        (200, "ready"),
        (503, "stopping"),
    ] {
        match reason {
            "database_unknown" => metrics.readiness.admit(),
            "database_unavailable" => {
                metrics
                    .readiness
                    .observe(Dependency::Database, false, Duration::ZERO)
            }
            "ready" => {
                metrics
                    .readiness
                    .observe(Dependency::Database, true, Duration::ZERO);
                metrics
                    .readiness
                    .observe(Dependency::Broker, true, Duration::ZERO);
            }
            "stopping" => metrics.readiness.stop(),
            _ => {}
        }
        for _ in 0..10 {
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            socket
                .write_all(b"GET /ready HTTP/1.1\r\nHost: local\r\n\r\n")
                .await
                .unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).await.unwrap();
            assert!(response.starts_with(&format!("HTTP/1.1 {code}")));
            assert!(response.contains(&format!("\"reason\":\"{reason}\"")));
            // No sampler exists: requests/rendering cannot invent observations or retry failures.
            assert_eq!(
                metrics.readiness.snapshot().failures[0],
                u64::from(matches!(
                    reason,
                    "database_unavailable" | "ready" | "stopping"
                ))
            );
            let rendered = metrics.render();
            assert!(!rendered.contains("backlog"));
        }
    }
    exporter.shutdown().await;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn sampler_lock_failure_recovery_and_single_flight() {
    support::isolated(|pool| async move {
        let state = Arc::new(Readiness::new(true, false));
        state.admit();
        let mut lock = pool.begin().await?;
        sqlx::query("LOCK TABLE relay.outbox_events IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await?;
        let observation = DatabaseObservation::start(
            (*pool.connect_options()).clone(),
            &["SELECT id FROM relay.outbox_events LIMIT 0"],
            state.clone(),
        );
        timeout(Duration::from_secs(4), async {
            loop {
                if state.snapshot().failures[0] > 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(state.snapshot().reason, Reason::DatabaseUnavailable);
        assert!(state.snapshot().last_success[0].is_none());
        assert!(state.snapshot().duration[0] <= Duration::from_secs(3));
        // The only additional sampler connection is capped at one, even while blocked.
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database()",
        )
        .fetch_one(&pool)
        .await?;
        assert!(n <= 3);
        lock.rollback().await?;
        timeout(Duration::from_secs(9), async {
            loop {
                if state.snapshot().ready() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        observation.shutdown().await;
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn sampler_overall_deadline_never_reports_success() {
    support::isolated(|pool| async move {
        let state = Arc::new(Readiness::new(true, false));
        state.admit();
        // Three individually bounded statements would exceed the 2s whole-sample deadline.
        let observation = DatabaseObservation::start(
            (*pool.connect_options()).clone(),
            &[
                "SELECT pg_sleep(0.8)",
                "SELECT pg_sleep(0.8)",
                "SELECT pg_sleep(0.8)",
            ],
            state.clone(),
        );
        timeout(Duration::from_secs(4), async {
            loop {
                if state.snapshot().failures[0] > 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(state.snapshot().reason, Reason::DatabaseUnavailable);
        assert!(state.snapshot().last_success[0].is_none());
        assert!(state.snapshot().duration[0] >= Duration::from_secs(2));
        assert!(state.snapshot().duration[0] < Duration::from_secs(3));
        observation.shutdown().await;
        Ok(())
    })
    .await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn enabled_replay_database_degradation_preserves_http_liveness_and_recovers() {
    support::isolated(|pool|async move {
        for sql in [include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql"),include_str!("../migrations/20261012000000_add_durable_retry.up.sql"),include_str!("../migrations/20261013000000_add_audited_replay.up.sql")] {
            sqlx::raw_sql(sql).execute(&pool).await?;
        }
        let database:String=sqlx::query_scalar("SELECT current_database()").fetch_one(&pool).await?;
        let reserve=std::net::TcpListener::bind("127.0.0.1:0")?;let http=reserve.local_addr()?;drop(reserve);
        let mut command=tokio::process::Command::new(env!("CARGO_BIN_EXE_reliable-event-relay"));
        let addresses=support::metrics_env(&mut command);
        // Disposable fixture credential; no application requests use it.
        let operators=serde_json::json!([{"token":"a".repeat(64),"actor":"readiness-test","allow_replay":true,"maximum_budget":1}]).to_string();
        let mut child=command.env("POSTGRES_DB",database).env("HTTP_ADDR",http.to_string()).env("REPLAY_OPERATORS",operators)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).kill_on_drop(true).spawn()?;
        let result=async {
            support::wait_ready(addresses[0],"http").await;
            let mut lock=pool.begin().await?;
            sqlx::query("LOCK TABLE relay.replay_requests IN ACCESS EXCLUSIVE MODE").execute(&mut *lock).await?;
            timeout(Duration::from_secs(9),async {loop {
                let mut socket=tokio::net::TcpStream::connect(addresses[0]).await.unwrap();
                socket.write_all(b"GET /ready HTTP/1.1\r\nHost: local\r\n\r\n").await.unwrap();
                let mut out=String::new();socket.read_to_string(&mut out).await.unwrap();
                if out.starts_with("HTTP/1.1 503") && out.contains("database_unavailable") {break;}
                tokio::time::sleep(Duration::from_millis(20)).await;
            }}).await?;
            let mut socket=tokio::net::TcpStream::connect(http).await?;
            socket.write_all(b"GET /health HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n").await?;
            let mut out=String::new();timeout(Duration::from_secs(2),socket.read_to_string(&mut out)).await??;
            assert!(out.starts_with("HTTP/1.1 200 OK"));assert!(out.ends_with("ok\n"));
            lock.rollback().await?;
            support::wait_ready(addresses[0],"http").await;
            Ok::<(),Box<dyn std::error::Error+Send+Sync>>(())
        }.await;
        tokio::process::Command::new("kill").args(["-TERM",&child.id().unwrap().to_string()]).status().await?;
        assert!(timeout(Duration::from_secs(12),child.wait()).await??.success());
        assert!(tokio::net::TcpStream::connect(addresses[0]).await.is_err());
        result?;Ok(())
    }).await;
}
