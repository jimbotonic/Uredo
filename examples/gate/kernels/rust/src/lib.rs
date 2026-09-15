//! Hand-written Rust twin of `../../uredo/src/lib.ure` (gate item 7).

pub mod keep;

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut acc = 0.0;
    for i in 0..a.len().min(b.len()) {
        acc += a[i] * b[i];
    }
    acc
}

pub fn count_words(s: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for b in s.bytes() {
        if b == b' ' || b == b'\n' || b == b'\t' {
            in_word = false;
        } else if !in_word {
            in_word = true;
            count += 1;
        }
    }
    count
}
