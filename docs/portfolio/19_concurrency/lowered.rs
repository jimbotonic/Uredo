//! 19 — Concurrency: threads, `move` closures, channels, `Arc<Mutex<T>>`, scoped threads.
//! `Send`/`Sync` stay Rust's; Uredo neither hides nor bypasses them (§21).

use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;

fn main() {
    let handles: Vec<thread::JoinHandle<u64>> = (0..4u64)
        .map(|i| thread::spawn(move || (i * 1000..(i + 1) * 1000).sum::<u64>()))
        .collect();
    let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    ::std::println!("total {total}");

    let (tx, rx) = mpsc::channel::<String>();
    for id in 0..3 {
        let tx = tx.clone();
        thread::spawn(move || tx.send(::std::format!("worker {id} done")).unwrap());
    }
    drop(tx);
    let mut messages: Vec<String> = rx.iter().collect();
    messages.sort();
    ::std::println!("{messages:?}");

    let counter = Arc::new(Mutex::new(0u32));
    let workers: Vec<thread::JoinHandle<()>> = (0..8)
        .map(|_| {
            let counter = Arc::clone(&counter);
            thread::spawn(move || {
                let mut guard = counter.lock().unwrap();
                *guard += 1;
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    ::std::println!("counter {}", *counter.lock().unwrap());

    let config = Arc::new(RwLock::new(String::from("v1")));
    *config.write().unwrap() = String::from("v2");
    ::std::println!("config {}", *config.read().unwrap());

    let data = Vec::from([1, 2, 3, 4, 5, 6]);
    let (left, right) = data.split_at(3);
    thread::scope(|s| {
        s.spawn(|| ::std::println!("left sum {}", left.iter().sum::<i32>()));
        let _ = s.spawn(|| ::std::println!("right sum {}", right.iter().sum::<i32>()));
    });
}
