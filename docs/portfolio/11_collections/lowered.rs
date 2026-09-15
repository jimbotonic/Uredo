//! 11 — Collections: `Vec`, `HashMap`, `HashSet`, the entry API, and the three
//! ways a `for` loop can visit a collection.

use std::collections::{HashMap, HashSet};

fn main() {
    let mut words: Vec<String> = Vec::new();
    for w in "the quick brown fox jumps over the lazy dog the end".split(' ') {
        words.push(w.to_string());
    }

    for w in &words {
        if w.len() > 4 {
            ::std::println!("long: {w}");
        }
    }
    for w in &mut words {
        w.make_ascii_uppercase();
    }
    ::std::println!("{words:?}");

    let mut counts: HashMap<String, usize> = HashMap::new();
    for w in &words {
        *counts.entry(w.clone()).or_insert(0) += 1;
    }
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort();
    ::std::println!("{pairs:?}");

    let mut seen: HashSet<u32> = HashSet::new();
    for n in [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5] {
        if !seen.insert(n) {
            ::std::println!("duplicate {n}");
        }
    }
    ::std::println!("{}", seen.contains(&9));

    let mut lengths = Vec::new();
    for w in words {
        lengths.push(w.len());
    }
    lengths.sort_unstable();
    lengths.dedup();
    ::std::println!("{lengths:?}");
}
