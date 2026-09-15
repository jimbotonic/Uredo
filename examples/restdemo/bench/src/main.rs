//! A load generator for `examples/restdemo`, written here rather than taken off the shelf so the
//! measurement is reproducible from this repository and so the generator's own ceiling can be
//! measured alongside the thing it measures.
//!
//! Keep-alive connections, a fixed request, `Content-Length`-framed reads, one latency sample per
//! request. It reports a distribution rather than a single throughput number, because two servers
//! within noise of each other is the expected result and a mean would hide it.
//!
//!   restdemo_bench <host:port> <path> <connections> <seconds>

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        eprintln!("usage: restdemo_bench <host:port> <path> <connections> <seconds>");
        std::process::exit(2);
    }
    let (addr, path) = (a[0].clone(), a[1].clone());
    let conns: usize = a[2].parse().expect("connections");
    let secs: u64 = a[3].parse().expect("seconds");

    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut tasks = Vec::new();
    for _ in 0..conns {
        let (addr, path) = (addr.clone(), path.clone());
        tasks.push(tokio::spawn(async move { drive(&addr, &path, deadline).await }));
    }

    let mut latencies: Vec<u64> = Vec::new();
    let mut errors = 0u64;
    for t in tasks {
        match t.await.expect("join") {
            Ok(mut v) => latencies.append(&mut v),
            Err(n) => errors += n,
        }
    }
    if latencies.is_empty() {
        eprintln!("no successful requests ({errors} errors)");
        std::process::exit(1);
    }
    latencies.sort_unstable();
    let n = latencies.len();
    // Latencies are nanoseconds; report microseconds, which is the scale these actually sit at.
    let at = |q: f64| latencies[((n as f64 - 1.0) * q).round() as usize] as f64 / 1000.0;
    // Throughput over the wall clock the run actually occupied, not over `secs`.
    let rps = n as f64 / secs as f64;
    println!(
        "requests {n:>7}  errors {errors}  rps {rps:>6.0}  p50 {:>7.1}us  p90 {:>7.1}us  p99 {:>8.1}us",
        at(0.50),
        at(0.90),
        at(0.99)
    );
}

/// One keep-alive connection, issuing requests back to back until the deadline.
async fn drive(addr: &str, path: &str, deadline: Instant) -> Result<Vec<u64>, u64> {
    let mut stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(_) => return Err(1),
    };
    let _ = stream.set_nodelay(true);
    let req = format!("GET {path} HTTP/1.1\r\nHost: b\r\n\r\n");
    let mut samples = Vec::with_capacity(4096);
    let mut buf = vec![0u8; 16 * 1024];
    let mut held = Vec::with_capacity(16 * 1024);
    let mut errors = 0u64;

    while Instant::now() < deadline {
        let start = Instant::now();
        if stream.write_all(req.as_bytes()).await.is_err() {
            errors += 1;
            break;
        }
        match read_one(&mut stream, &mut buf, &mut held).await {
            Ok(()) => samples.push(start.elapsed().as_nanos() as u64),
            Err(()) => {
                errors += 1;
                break;
            }
        }
    }
    if samples.is_empty() { Err(errors.max(1)) } else { Ok(samples) }
}

/// Read exactly one response: headers, then `Content-Length` bytes of body.
async fn read_one(stream: &mut TcpStream, buf: &mut [u8], held: &mut Vec<u8>) -> Result<(), ()> {
    loop {
        if let Some(head_end) = find(held, b"\r\n\r\n") {
            let head = &held[..head_end];
            let len = content_length(head);
            let total = head_end + 4 + len;
            if held.len() >= total {
                held.drain(..total);
                return Ok(());
            }
        }
        let n = stream.read(buf).await.map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        held.extend_from_slice(&buf[..n]);
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn content_length(head: &[u8]) -> usize {
    let text = String::from_utf8_lossy(head);
    for line in text.split("\r\n") {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                return v.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}
