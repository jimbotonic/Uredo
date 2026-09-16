//! The floor: a hyper server that does nothing but return a fixed body.
//!
//! Without this the service's numbers are uninterpretable. If the load generator saturates at
//! 170k requests per second then a service measured at 166k is being reported as the generator's
//! ceiling, not its own. This is the same shape as a "plaintext" benchmark, on this machine,
//! under this generator — which is the only reference point that transfers.

use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let listen = std::env::var("RESTDEMO_ADDR").unwrap_or_else(|_| "127.0.0.1:8081".to_string());
    let addr: SocketAddr = listen.parse().expect("address");
    let listener = TcpListener::bind(addr).await.expect("bind");
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(|_req: Request<hyper::body::Incoming>| async {
                        Ok::<_, std::convert::Infallible>(
                            Response::builder()
                                .header("content-type", "application/json")
                                .body(Full::new(Bytes::from_static(b"{\"status\":\"ok\"}")))
                                .unwrap(),
                        )
                    }),
                )
                .await;
        });
    }
}
