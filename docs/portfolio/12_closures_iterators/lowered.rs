//! 12 — Closures and iterators: `x => …`, capture modes, `move`, lazy chains,
//! explicit materialisation with `collect`.

#[derive(Debug, Clone)]
struct Item {
    name: String,
    price_cents: u32,
    in_stock: bool,
}

/// A function taking a closure: the bound is Rust's; `impl Fn` is an anonymous type parameter
/// and passes by value (P8, D52), so the caller hands the closure over as Rust does.
fn apply_twice(f: impl Fn(u32) -> u32, x: u32) -> u32 {
    f(f(x))
}

fn main() {
    let items = Vec::from([
        Item {
            name: String::from("pen"),
            price_cents: 150,
            in_stock: true,
        },
        Item {
            name: String::from("ink"),
            price_cents: 900,
            in_stock: false,
        },
        Item {
            name: String::from("pad"),
            price_cents: 450,
            in_stock: true,
        },
    ]);

    let double = |x: u32| -> u32 { x * 2 };
    ::std::println!("{}", apply_twice(double, 5));
    ::std::println!("{}", apply_twice(|x| x + 1, 5));

    let names: Vec<&str> = items
        .iter()
        .filter(|i| i.in_stock)
        .map(|i| i.name.as_str())
        .collect();
    ::std::println!("{names:?}");
    let total = items.iter().map(|i| i.price_cents).sum::<u32>();
    ::std::println!("total {total}");
    let cheapest = items
        .iter()
        .min_by_key(|i| i.price_cents)
        .map(|i| i.name.clone());
    ::std::println!("{cheapest:?}");

    let mut calls = 0;
    let mut log = || calls += 1;
    log();
    log();
    ::std::println!("calls {calls}");

    let prefix = String::from("item: ");
    let label = move |name: &str| ::std::format!("{prefix}{name}");
    ::std::println!("{}", label("pen"));

    let summary = items.iter().fold(String::new(), |mut acc, item| {
        acc.push_str(&item.name);
        acc.push(' ');
        acc
    });
    ::std::println!("{summary}");

    for (i, item) in items.iter().enumerate() {
        ::std::println!("{i}: {}", item.name);
    }
    for pair in [1, 2, 3, 4].windows(2) {
        ::std::println!("{pair:?}");
    }
}
