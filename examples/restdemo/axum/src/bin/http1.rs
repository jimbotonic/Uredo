//! The same axum `Router`, served over http1 alone.
//!
//! `axum::serve` builds on hyper-util's *auto* connection builder, which sniffs the first bytes
//! of every connection to tell http1 from http2 prior-knowledge. The hand-rolled twin does not:
//! it names `http1::Builder` and serves that. So a throughput gap between them is not
//! necessarily the framework — it could be the sniffing, and attributing one to the other
//! without checking would be the whole point of this comparison thrown away.
//!
//! This binary holds the framework constant and removes only the auto-detection. What is left
//! between it and the twin is the router, the extractors and the tower stack.

use std::sync::Arc;

use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use restdemo_idiomatic::store::Store;
use tokio::net::TcpListener;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let listen = std::env::var("RESTDEMO_ADDR").unwrap_or_else(|_e| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&listen).await.expect("bind");
    let app = restdemo_axum::app(Arc::new(Store::new()));
    println!("restdemo listening on http://{listen}");
    loop {
        let Ok((stream, _peer)) = listener.accept().await else { continue };
        let service = TowerToHyperService::new(app.clone());
        tokio::spawn(async move {
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
        });
    }
}
