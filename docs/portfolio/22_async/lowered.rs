//! 22 — Async: `async fn`, postfix `.await`, `.await?`, spawning tasks, channels.
//! Uredo ships no runtime: Tokio is an ordinary dependency, entered through a forwarded
//! attribute (§21). Async surface syntax is Phase 2 in the plan; the rules are the same as sync.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

#[derive(Debug)]
struct Weather {
    city: String,
    temp_c: f64,
}

/// An `async fn` returning `throws`: the body awaits with postfix `.await`; `?` composes (D30).
async fn fetch_weather(city: &str) -> ::core::result::Result<Weather, String> {
    sleep(Duration::from_millis(10)).await;
    if city.is_empty() {
        return ::core::result::Result::Err(String::from("no city"));
    }
    ::core::result::Result::Ok(Weather {
        city: city.to_string(),
        temp_c: 21.5,
    })
}

/// Awaiting several futures concurrently: `tokio::join!` is a macro, invoked directly (D23).
async fn both() -> ::core::result::Result<(Weather, Weather), String> {
    let (a, b) = tokio::join!(fetch_weather("Oslo"), fetch_weather("Lima"));
    ::core::result::Result::Ok((a?, b?))
}

/// `@rust(tokio::main)` forwards the runtime attribute unchanged (§21).
#[tokio::main]
async fn main() -> ::core::result::Result<(), String> {
    let w = fetch_weather("Bern").await?;
    ::std::println!("{w:?} ({} °C)", w.temp_c);
    ::std::println!("{:?}", fetch_weather("").await);
    let (a, b) = both().await?;
    ::std::println!("{} {}", a.city, b.city);

    let (tx, rx) = mpsc::channel::<u32>(8);
    let mut rx = rx;
    for i in 0..3u32 {
        let tx = tx.clone();
        tokio::spawn(async move { tx.send(i * i).await.unwrap() });
    }
    drop(tx);
    let mut got = Vec::new();
    while let Some(v) = rx.recv().await {
        got.push(v);
    }
    got.sort();
    ::std::println!("{got:?}");
    ::core::result::Result::Ok(())
}
