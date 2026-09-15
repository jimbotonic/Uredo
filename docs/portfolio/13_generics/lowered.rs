//! 13 — Generics: type parameters, bounds, `where`, generic structs and impls.
//!
//! The passing mode of a type parameter is decided syntactically (D1, D52): `x: T` passes
//! *by value* (P8) — the type-parameter form is the visible sign, as the pattern is in P7 —
//! and a `?Sized` bound keeps the borrow. Write `&T` to borrow a type parameter explicitly.

/// A generic function with an inline bound. `values: [T]` is `&[T]`; the return borrows from it.
fn largest<T: PartialOrd>(values: &[T]) -> ::core::option::Option<&T> {
    let mut best = values.first()?;
    for v in values {
        if v > best {
            best = v;
        }
    }
    Some(best)
}

/// A type parameter is already by value (P8); `take T` is accepted and means the same (a lint
/// will flag it as redundant), so the plain form is preferred.
fn boxed<T>(value: T) -> Box<T> {
    Box::new(value)
}

/// `where` clauses are Rust's, written as an indented block.
fn describe_all<T, U>(items: &[T], tag: U) -> Vec<String>
where
    T: std::fmt::Debug,
    U: std::fmt::Display + Copy,
{
    items
        .iter()
        .map(|i| ::std::format!("{tag}: {i:?}"))
        .collect()
}

/// A generic struct with an `impl<T>` block and a bounded method.
#[derive(Debug, Clone)]
struct Pair<T> {
    first: T,
    second: T,
}

impl<T: Clone + PartialOrd> Pair<T> {
    fn new(first: T, second: T) -> Self {
        Self { first, second }
    }

    fn max(&self) -> T {
        if self.first >= self.second {
            self.first.clone()
        } else {
            self.second.clone()
        }
    }

    fn swap(&mut self) {
        std::mem::swap(&mut self.first, &mut self.second);
    }
}

/// Const generics keep Rust's spelling (Phase 3 surface, shown for completeness).
fn sum_array<const N: usize>(xs: [i32; N]) -> i32 {
    xs.iter().sum()
}

fn main() {
    ::std::println!("{:?}", largest(&[3, 9, 2]));
    ::std::println!("{:?}", largest(&Vec::<f64>::new()));
    let b = boxed(String::from("on the heap"));
    ::std::println!("{b}");
    ::std::println!("{:?}", describe_all(&[1, 2], "n"));
    let mut p = Pair::new(String::from("x"), String::from("y"));
    ::std::println!("{}", p.max());
    p.swap();
    ::std::println!("{p:?}");
    ::std::println!("{}", sum_array([1, 2, 3, 4]));
}
