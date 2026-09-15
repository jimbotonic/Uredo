//! 17. Tokio program: tasks, an mpsc channel, a timeout, joining results.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

async fn worker(id: u32, tx: mpsc::Sender<String>) {
    for round in 0..3 {
        sleep(Duration::from_millis(1)).await;
        let _ = tx.send(format!("worker {id} round {round}")).await;
    }
}

async fn slow() -> u32 {
    sleep(Duration::from_millis(50)).await;
    42
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<String>(16);
    let mut handles = Vec::new();
    for id in 0..3 {
        handles.push(tokio::spawn(worker(id, tx.clone())));
    }
    drop(tx);
    for h in handles {
        h.await.expect("worker panicked");
    }
    let mut messages = Vec::new();
    while let Some(m) = rx.recv().await {
        messages.push(m);
    }
    messages.sort();
    println!("{} messages, first {:?}", messages.len(), messages[0]);
    match timeout(Duration::from_millis(5), slow()).await {
        Ok(v) => println!("got {v}"),
        Err(_) => println!("timed out"),
    }
    println!(
        "{}",
        timeout(Duration::from_millis(500), slow())
            .await
            .unwrap_or(0)
    );
}
