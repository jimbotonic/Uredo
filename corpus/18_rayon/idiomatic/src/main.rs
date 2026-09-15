//! 18. Rayon parallel loop: data-parallel iterators over a slice, a parallel sort, a reduction.

use rayon::prelude::*;

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 1;
    }
    true
}

fn count_primes(numbers: &[u64]) -> usize {
    numbers.par_iter().filter(|n| is_prime(**n)).count()
}

fn squares_sum(n: u64) -> u64 {
    (1..=n).into_par_iter().map(|i| i * i).sum()
}

fn main() {
    let numbers: Vec<u64> = (1..20000).collect();
    println!("{} primes below 20000", count_primes(&numbers));
    println!("sum of squares to 1000: {}", squares_sum(1000));
    let mut words: Vec<String> = ["pear", "apple", "fig", "date", "cherry"]
        .iter()
        .map(|w| w.to_string())
        .collect();
    words.par_sort();
    println!("{words:?}");
    let longest = words.par_iter().map(|w| w.len()).max().unwrap_or(0);
    println!("longest {longest}");
}
