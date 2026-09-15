//! 20. Inline optimisation: a safe version and an unsafe unchecked-indexing version.

fn checksum_safe(data: &[u8]) -> u32 {
    let mut acc: u32 = 0;
    for b in data {
        acc = acc.wrapping_mul(31).wrapping_add(*b as u32);
    }
    acc
}

/// The hot loop with `get_unchecked`: the `unsafe` is visible and local.
fn checksum_fast(data: &[u8]) -> u32 {
    let mut acc: u32 = 0;
    let mut i = 0;
    while i < data.len() {
        // SAFETY: i < data.len() is checked by the loop condition
        acc = acc
            .wrapping_mul(31)
            .wrapping_add(unsafe { *data.get_unchecked(i) } as u32);
        i += 1;
    }
    acc
}

pub fn popcount_all(words: &[u64]) -> u32 {
    words.iter().map(|w| w.count_ones()).sum()
}

fn main() {
    let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    let a = checksum_safe(&data);
    let b = checksum_fast(&data);
    println!("{a} {b} equal={}", a == b);
    println!("popcount {}", popcount_all(&[0xFFu64, 0x0F, 1]));
}
