pub mod publisher;
use axum::{Router, routing::get};
use std::{future::Future, io};
use tokio::net::TcpListener;

pub fn router() -> Router {
    Router::new().route("/health", get(|| async { "ok\n" }))
}

pub async fn serve(
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown)
        .await
}

pub async fn shutdown_signal() {
    let interrupt = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
        tokio::select! {
            () = interrupt => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    interrupt.await;
    tracing::info!("shutdown requested");
}
