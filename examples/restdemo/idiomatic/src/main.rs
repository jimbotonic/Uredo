//! The binary: bind a listener and hand it to the library.

use std::net::SocketAddr;
use std::sync::Arc;

use restdemo_idiomatic::error::Error;
use restdemo_idiomatic::store::Store;
use tokio::net::TcpListener;

/// Multi-threaded on purpose: a task per connection and an `RwLock` for reads.
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Error> {
    // The listen address comes from the environment, defaulting to 8080 — see the Uredo twin.
    let listen = std::env::var("RESTDEMO_ADDR").unwrap_or_else(|_e| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = listen.parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!("restdemo listening on http://{addr}");
    restdemo_idiomatic::serve(listener, Arc::new(Store::new())).await
}
