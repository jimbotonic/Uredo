//! 16. Async HTTP request: a local HTTP/1.0 server task and a client that GETs from it (offline).

use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn serve_one(listener: TcpListener) -> io::Result<()> {
    let (mut socket, _) = listener.accept().await?;
    let mut buf = vec![0u8; 1024];
    let n = socket.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let body = if request.starts_with("GET /hello") {
        "hello from uredo"
    } else {
        "not found"
    };
    let status = if request.starts_with("GET /hello") {
        "200 OK"
    } else {
        "404 Not Found"
    };
    let response = format!(
        "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn get(addr: &str, path: &str) -> io::Result<(u16, String)> {
    let mut stream = TcpStream::connect(addr).await?;
    stream
        .write_all(format!("GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes())
        .await?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).await?;
    let status: u16 = raw
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    Ok((status, body))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();
    let server = tokio::spawn(serve_one(listener));
    let (status, body) = get(&addr, "/hello").await?;
    println!("{status} {body}");
    server.await.expect("server task")?;
    Ok(())
}
