//! 14. Mutable borrowing: `&mut` parameters, `for … in &mut`, `&mut self`, slices.

#[derive(Debug, Default)]
struct Stats {
    count: usize,
    sum: i64,
}

impl Stats {
    fn add(&mut self, value: i64) {
        self.count += 1;
        self.sum += value;
    }
}

fn normalise(values: &mut [f64]) {
    let max = values.iter().cloned().fold(f64::MIN, f64::max);
    if max > 0.0 {
        for v in values.iter_mut() {
            *v /= max;
        }
    }
}

fn bump_all(counters: &mut Vec<u32>, by: u32) {
    for c in counters.iter_mut() {
        *c += by;
    }
    counters.push(by);
}

fn record(stats: &mut Stats, values: &[i64]) {
    for v in values {
        stats.add(*v);
    }
}

fn swap_ends(xs: &mut [i32]) {
    let n = xs.len();
    if n >= 2 {
        xs.swap(0, n - 1);
    }
}

fn main() {
    let mut values = [3.0, 1.5, 6.0];
    normalise(&mut values);
    println!("{values:?}");
    let mut counters = vec![1, 2, 3];
    bump_all(&mut counters, 10);
    println!("{counters:?}");
    let mut stats = Stats::default();
    record(&mut stats, &[4, 5, 6]);
    println!("{} values sum {}", stats.count, stats.sum);
    let mut xs = [1, 2, 3, 4];
    swap_ends(&mut xs);
    println!("{xs:?}");
}
