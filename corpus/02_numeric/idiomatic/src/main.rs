//! 2. Numeric computation: integer and float arithmetic, loops, a sieve, statistics.

fn gcd(a: u64, b: u64) -> u64 {
    let mut x = a;
    let mut y = b;
    while y != 0 {
        let t = y;
        y = x % y;
        x = t;
    }
    x
}

fn primes_below(limit: usize) -> Vec<usize> {
    let mut sieve = vec![true; limit];
    let mut primes = Vec::new();
    for n in 2..limit {
        if sieve[n] {
            primes.push(n);
            let mut m = n * n;
            while m < limit {
                sieve[m] = false;
                m += n;
            }
        }
    }
    primes
}

fn mean_and_variance(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let variance = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
    (mean, variance)
}

fn main() {
    println!("gcd(1071, 462) = {}", gcd(1071, 462));
    let primes = primes_below(50);
    println!(
        "{} primes below 50, last {}",
        primes.len(),
        primes[primes.len() - 1]
    );
    let (mean, variance) = mean_and_variance(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    println!(
        "mean {mean:.2} variance {variance:.2} sd {:.2}",
        variance.sqrt()
    );
    let total: u32 = (1..=100u32).filter(|n| n % 3 == 0 || n % 5 == 0).sum();
    println!("multiples of 3 or 5 below 101 sum to {total}");
}
