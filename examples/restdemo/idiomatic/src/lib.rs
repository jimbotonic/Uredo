//! An opinionated JSON REST service: fixed routes, a fixed type set, no framework.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

pub mod error;
pub mod etag;
pub mod field;
pub mod handler;
pub mod model;
pub mod query;
pub mod router;
pub mod store;
pub mod wire;

use error::Error;
use store::Store;

/// How long a shutdown waits for in-flight requests before giving up on them.
pub const DRAIN: Duration = Duration::from_secs(5);

/// Serve until the process is killed.
pub async fn serve(listener: TcpListener, state: Arc<Store>) -> Result<(), Error> {
    serve_until(listener, state, Arc::new(Notify::new())).await
}

/// Serve until `shutdown` is notified, then stop accepting and let in-flight work finish.
pub async fn serve_until(
    listener: TcpListener,
    state: Arc<Store>,
    shutdown: Arc<Notify>,
) -> Result<(), Error> {
    let inflight = Arc::new(AtomicUsize::new(0));
    loop {
        let accepted: Option<Result<(TcpStream, SocketAddr), std::io::Error>> = tokio::select! {
            r = listener.accept() => Some(r),
            _ = shutdown.notified() => None,
        };
        let Some(result) = accepted else { break };
        let (stream, _peer) = result?;
        let conn_state = Arc::clone(&state);
        let counter = Arc::clone(&inflight);
        counter.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            if let Err(e) = wire::connection(stream, conn_state).await {
                eprintln!("connection error: {e}");
            }
            counter.fetch_sub(1, Ordering::Relaxed);
        });
    }
    drain(&inflight).await
}

/// Wait for the in-flight count to reach zero, or for `DRAIN` to pass.
async fn drain(inflight: &Arc<AtomicUsize>) -> Result<(), Error> {
    let deadline = Instant::now() + DRAIN;
    while inflight.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Ok(())
}
