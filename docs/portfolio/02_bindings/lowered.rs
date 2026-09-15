//! 02 — Bindings, mutability, shadowing, constants, literals and casts.

const MAX_RETRIES: u32 = 3;
static BANNER: &'static str = "uredo";

/// A declaration group (D46): one visibility, an ordered list of ordinary `const` items. There is
/// no group item in the generated Rust; an attribute on the group would apply to every member.
const KILO: u64 = 1000;
const MEGA: u64 = KILO * 1000;

fn main() {
    let count = 10;
    let mut total = 0;
    total = total + count;
    total += 1;

    let input = "  42  ";
    let input = input.trim();
    let input: u32 = input.parse().unwrap();
    ::std::println!("parsed {input}, total {total}, retries {MAX_RETRIES}, banner {BANNER}");

    let ratio: f64 = 0.5;
    let a = 42;
    let b = 42u64;
    let c = 3.14;
    let d = 0xFF;
    let e = 1_000_000;

    let small = e as u16;
    let frac = a as f64 / 8.0;

    let (lo, hi) = (1, 100);
    let (mut x, mut y) = (0.0, 1.0);
    (x, y) = (y, x);
    let _ = ratio;

    ::std::println!("{a} {b} {c} {d} {e} {small} {frac} {lo}..{hi} ({x}, {y})");

    let wrapped = u8::MAX.wrapping_add(1);
    let checked = u8::MAX.checked_add(1);
    ::std::println!("{wrapped} {checked:?}");
    ::std::println!("{} {}", KILO, MEGA);
}
