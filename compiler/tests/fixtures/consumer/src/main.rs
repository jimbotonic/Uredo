//! A plain Rust program using the package exported by `uredo package` (§4.6, gate item 5).
use geometry::shapes::{Circle, describe};
use geometry::{Rect, total_area};

fn main() {
    let mut r = Rect::new(2.0, 3.0);
    r.scale(2.0);
    let c = Circle { r: 1.0, label: String::from("unit") };
    println!("{} {} {}", r.area(), total_area(&[r, Rect::new(1.0, 1.0)]), describe(&c));
}
