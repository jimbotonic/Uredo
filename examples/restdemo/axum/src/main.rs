//! The binary: bind a listener and hand it to the library.

use std::sync::Arc;

use restdemo_idiomatic::error::Error;
use restdemo_idiomatic::store::Store;
use tokio::net::TcpListener;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Error> {
    // The same environment variable as the other two, so one benchmark harness drives all three.
    let listen = std::env::var("RESTDEMO_ADDR").unwrap_or_else(|_e| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&listen).await?;
    println!("restdemo listening on http://{listen}");
    restdemo_axum::serve(listener, Arc::new(Store::new())).await
}
