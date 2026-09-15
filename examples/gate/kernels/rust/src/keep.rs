//! Forces codegen of every kernel in the rlib (identical on both sides, outside the compared region).
use super::*;
#[inline(never)]
pub fn __keep() {
    let a: fn(&[f64], &[f64]) -> f64 = dot;
    let b: fn(&str) -> usize = count_words;
    std::hint::black_box((a, b));
}
