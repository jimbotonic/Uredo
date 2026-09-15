//! Plain Rust consuming the exported mixed `.ure`/`.rs` gate package (§34 item 5): one item from
//! the hand-written Rust module, one from generated Rust.
use gate::route::route;
use gate::{Job, spawn_len};

fn main() {
    let n = 3u8;
    let job = Job::new(9, "abcdef");
    println!("{} {} {}", route(n), route(&n), spawn_len(job));
}
