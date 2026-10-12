#![allow(dead_code)]
fn checked<T>(result: Result<T, sqlx::Error>, operation: &str) -> T {
    result.unwrap_or_else(|_| panic!("{operation} failed (source redacted)"))
}
use sqlx::PgPool;
use std::time::Duration;
pub fn uri() -> String {
    // Use the same validated escaped environment configuration for test-only consumers.
    let mut uri = url::Url::parse("amqp://rabbitmq/relay").unwrap();
    uri.set_username(&std::env::var("RABBITMQ_DEFAULT_USER").unwrap())
        .unwrap();
    uri.set_password(Some(&std::env::var("RABBITMQ_DEFAULT_PASS").unwrap()))
        .unwrap();
    uri.set_host(Some(
        &std::env::var("RABBITMQ_HOST").unwrap_or_else(|_| "rabbitmq".into()),
    ))
    .unwrap();
    uri.set_port(Some(
        std::env::var("RABBITMQ_PORT")
            .unwrap_or_else(|_| "5672".into())
            .parse()
            .unwrap(),
    ))
    .unwrap();
    uri.path_segments_mut()
        .unwrap()
        .clear()
        .push(&std::env::var("RABBITMQ_DEFAULT_VHOST").unwrap_or_else(|_| "relay".into()));
    uri.into()
}

// Test-owned plaintext PostgreSQL protocol proxy. It discards CommandComplete
// COMMIT rather than forwarding it to SQLx; shared services are never interrupted.
pub async fn lost_commit_ack_pool(pool: &PgPool) -> (PgPool, tokio::task::JoinHandle<bool>) {
    use sqlx::postgres::{PgPoolOptions, PgSslMode};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    let original = pool.connect_options();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("proxy bind");
    let port = listener.local_addr().unwrap().port();
    let host = original.get_host().to_owned();
    let upstream_port = original.get_port();
    let task = tokio::spawn(async move {
        let (client, _) = listener.accept().await.expect("proxy client");
        let server = TcpStream::connect((host.as_str(), upstream_port))
            .await
            .expect("proxy upstream");
        let (mut client_read, mut client_write) = client.into_split();
        let (mut server_read, mut server_write) = server.into_split();
        let frontend = tokio::io::copy(&mut client_read, &mut server_write);
        let backend = async {
            loop {
                let mut header = [0u8; 5];
                if server_read.read_exact(&mut header).await.is_err() {
                    return false;
                }
                let length = u32::from_be_bytes(header[1..].try_into().unwrap());
                assert!((4..=16_777_216).contains(&length), "bounded protocol frame");
                let mut body = vec![0; usize::try_from(length - 4).unwrap()];
                if server_read.read_exact(&mut body).await.is_err() {
                    return false;
                }
                if header[0] == b'C' && body == b"COMMIT\0" {
                    return true;
                }
                if client_write.write_all(&header).await.is_err()
                    || client_write.write_all(&body).await.is_err()
                {
                    return false;
                }
            }
        };
        tokio::select! {
            _=frontend=>false,
            committed=backend=>committed,
        }
    });
    let options = (*original)
        .clone()
        .host("127.0.0.1")
        .port(port)
        .ssl_mode(PgSslMode::Disable);
    let proxy_pool = checked(
        PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await,
        "connect isolated proxy pool",
    );
    (proxy_pool, task)
}

/// Withhold broker-to-client bytes after setup on a test-owned TCP proxy.
/// This changes no shared broker state or other clients' connections.
pub async fn proxy() -> (
    String,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    tokio::task::JoinHandle<()>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut url = url::Url::parse(&uri()).unwrap();
    let target = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(5672));
    url.set_host(Some("127.0.0.1")).unwrap();
    url.set_port(Some(address.port())).unwrap();
    let hold = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = hold.clone();
    let task = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let broker = tokio::net::TcpStream::connect(target).await.unwrap();
        let (mut client_read, mut client_write) = client.into_split();
        let (mut broker_read, mut broker_write) = broker.into_split();
        let outgoing = async {
            let _ = tokio::io::copy(&mut client_read, &mut broker_write).await;
        };
        let incoming = async {
            let mut buffer = [0; 8192];
            loop {
                let size = broker_read.read(&mut buffer).await.unwrap_or(0);
                if size == 0 {
                    break;
                }
                if flag.load(std::sync::atomic::Ordering::Acquire) {
                    std::future::pending::<()>().await;
                }
                if client_write.write_all(&buffer[..size]).await.is_err() {
                    break;
                }
            }
        };
        tokio::select! { () = outgoing => {}, () = incoming => {} }
    });
    (url.into(), hold, task)
}
