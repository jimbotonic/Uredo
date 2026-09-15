//! 03 — Functions and control flow: tail expressions, `if` as an expression,
//! loops, ranges, labelled breaks, inline bodies.

/// Parameters of built-in scalar types pass by value (rule P1). The last expression is
/// the return value (§8.4); `return` is available anywhere.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// `if`/`else` is an expression when every branch produces a value (§7.5).
/// A body of exactly one expression may sit on the header line (D34).
/// Return type: `str` would mean `&str` with an *elided* lifetime (§9.7), but there is no
/// borrowed parameter to borrow from, so the lifetime must be written: `&'static str` (rule P6).
fn sign(n: i32) -> &'static str {
    if n < 0 {
        "negative"
    } else if n == 0 {
        "zero"
    } else {
        "positive"
    }
}

/// `match` is also an expression; one-line arms are `pattern: expr`.
fn describe(n: u32) -> &'static str {
    match n {
        0 => "none",
        1 => "one",
        2 | 3 => "a few",
        4..=9 => "several",
        _ => "many",
    }
}

/// Early return with an inline guard body. The whole `if` is one statement.
fn clamp_percent(p: i32) -> i32 {
    if p < 0 {
        return 0;
    }
    if p > 100 {
        return 100;
    }
    p
}

fn main() {
    ::std::println!("{}", add(2, 3));
    ::std::println!("{} {} {}", sign(-5), sign(0), describe(7));
    ::std::println!("{}", clamp_percent(140));

    let mut sum = 0;
    for i in 1..=10 {
        sum += i;
    }
    ::std::println!("sum {sum}");

    let mut n = 27;
    let mut steps = 0;
    while n != 1 {
        n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
        steps += 1;
    }
    ::std::println!("collatz steps {steps}");

    let found = loop {
        steps += 1;
        if steps > 120 {
            break steps;
        }
    };
    ::std::println!("{found}");

    'outer: for i in 0..10 {
        for j in 0..10 {
            if i * j == 42 {
                ::std::println!("{i} * {j}");
                break 'outer;
            }
        }
    }
}
