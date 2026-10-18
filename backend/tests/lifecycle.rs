use reliable_event_relay::application;
use std::io::{Read, Write};
use std::time::Duration;

#[tokio::test]
async fn health_and_graceful_shutdown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(application::serve(listener, async {
        let _ = shutdown_rx.await;
    }));
    let response = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    })
    .await
    .unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.ends_with("ok\n"));
    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    // Probe asynchronously within the shutdown budget instead of assuming one
    // immediate synchronous connect observes the final socket state.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match tokio::net::TcpStream::connect(address).await {
                Err(_) => break,
                Ok(stream) => drop(stream),
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("listener remained available after shutdown");
}

#[cfg(unix)]
#[test]
fn binary_loads_dotenv_and_handles_signals() {
    use std::{
        fs,
        process::{Command, Stdio},
        thread,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    struct OwnedChild(std::process::Child);
    impl std::ops::Deref for OwnedChild {
        type Target = std::process::Child;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for OwnedChild {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for signal in ["-TERM", "-INT"] {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("relay-test-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join(".env"),
            "APP_ENV=dotenv-test\nHTTP_ADDR=127.0.0.1:0\nRUST_LOG=info\n",
        )
        .unwrap();
        let log_path = directory.join("output.log");
        let reserve = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let metrics_address = reserve.local_addr().unwrap();
        drop(reserve);
        let mut child = OwnedChild(
            Command::new(env!("CARGO_BIN_EXE_reliable-event-relay"))
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HTTP_METRICS_ADDR", metrics_address.to_string())
                .current_dir(&directory)
                .env_remove("APP_ENV")
                .env_remove("HTTP_ADDR")
                .env_remove("RUST_LOG")
                .stdout(Stdio::from(fs::File::create(&log_path).unwrap()))
                .stderr(Stdio::from(
                    fs::OpenOptions::new().append(true).open(&log_path).unwrap(),
                ))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let logs = fs::read_to_string(&log_path).unwrap();
            if logs.contains("application started") {
                assert!(logs.contains("dotenv-test"));
                break;
            }
            if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                let _ = child.kill();
                let _ = child.wait();
                panic!("application did not start: {logs}");
            }
            thread::sleep(Duration::from_millis(20));
        }
        // Allow the server to poll and register signal handlers after its startup log.
        thread::sleep(Duration::from_millis(100));
        let mut socket = std::net::TcpStream::connect(metrics_address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut metrics = String::new();
        socket.read_to_string(&mut metrics).unwrap();
        assert!(metrics.starts_with("HTTP/1.1 200 OK"));
        assert!(metrics.contains("process=\"http\""));
        assert!(metrics.contains("relay_ready{process=\"http\"} 1"));
        let mut ready = std::net::TcpStream::connect(metrics_address).unwrap();
        ready
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        ready
            .write_all(b"GET /ready HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        ready.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"state\":\"ready\""));
        assert!(
            Command::new("kill")
                .args([signal, &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("graceful shutdown timed out");
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            fs::read_to_string(&log_path)
                .unwrap()
                .contains("application stopped")
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
