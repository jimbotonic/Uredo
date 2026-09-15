//! 21 — Implementing `Iterator` and `IntoIterator`; associated types; generic traits
//! with associated types; iterator adaptors written by hand.

/// A hand-written iterator: state in a struct, `next` on `self: inout`, `type Item` as Rust.
struct Fib {
    a: u64,
    b: u64,
}

impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> ::core::option::Option<u64> {
        let out = self.a;
        self.a = self.b;
        self.b = out + self.b;
        Some(out)
    }
}

fn fib() -> Fib {
    Fib { a: 0, b: 1 }
}

/// A collection type that can be looped over: `IntoIterator` for the borrowed form.
struct Deck {
    cards: Vec<u8>,
}

impl Deck {
    /// `impl<'a> IntoIterator for &'a Deck` has its own generic parameter, so it lives at module
    /// level (D45 covers impls for the enclosing type without extra generics).
    fn len(&self) -> usize {
        self.cards.len()
    }
}

impl<'a> IntoIterator for &'a Deck {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;
    fn into_iter(self) -> Self::IntoIter {
        self.cards.iter()
    }
}

/// A generic trait with an associated type (Rust syntax throughout, §19).
trait Container {
    type Item;
    fn get(&self, i: usize) -> ::core::option::Option<Self::Item>;
    fn first(&self) -> ::core::option::Option<Self::Item> {
        self.get(0)
    }
}

impl Container for Deck {
    type Item = u8;
    fn get(&self, i: usize) -> ::core::option::Option<u8> {
        self.cards.get(i).copied()
    }
}

/// A generic adaptor consumes its input, and a type parameter passes by value already (P8, D52),
/// so no `take` is written — `uredo lint` reports one here as redundant.
fn every_other<I: Iterator>(it: I) -> impl Iterator<Item = I::Item> {
    it.step_by(2)
}

fn main() {
    let firsts: Vec<u64> = fib().take(10).collect();
    ::std::println!("{firsts:?}");
    ::std::println!("{:?}", fib().filter(|n| n % 2 == 0).nth(4));
    let deck = Deck {
        cards: Vec::from([7, 8, 9, 10]),
    };
    for c in &deck {
        ::std::println!("card {c}");
    }
    ::std::println!("{} {:?} {:?}", deck.len(), deck.first(), deck.get(9));
    let evens: Vec<u64> = every_other(fib()).take(5).collect();
    ::std::println!("{evens:?}");
}
