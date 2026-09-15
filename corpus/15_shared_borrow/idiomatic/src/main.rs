//! 15. Shared borrowing: read-only parameters, returned borrows with elision, `&str`/`&[T]`.

#[derive(Debug)]
struct Inventory {
    items: Vec<(String, u32)>,
}

fn first_word(s: &str) -> &str {
    s.split(' ').next().unwrap_or("")
}

fn cheapest(inv: &Inventory) -> Option<&(String, u32)> {
    inv.items.iter().min_by_key(|item| item.1)
}

fn total(inv: &Inventory) -> u32 {
    inv.items.iter().map(|i| i.1).sum()
}

fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

fn names(inv: &Inventory) -> Vec<&str> {
    inv.items.iter().map(|i| i.0.as_str()).collect()
}

fn contains(words: &[&str], needle: &str) -> bool {
    words.iter().any(|w| *w == needle)
}

fn main() {
    let inv = Inventory {
        items: vec![
            (String::from("bolt"), 3),
            (String::from("nut"), 1),
            (String::from("gear"), 12),
        ],
    };
    println!("{:?}", cheapest(&inv));
    println!("total {}", total(&inv));
    println!("{}", first_word("hello borrowed world"));
    println!("{}", longest("short", "longer one"));
    let ns = names(&inv);
    println!("{ns:?} has nut: {}", contains(&ns, "nut"));
    println!("{:?}", inv);
}
