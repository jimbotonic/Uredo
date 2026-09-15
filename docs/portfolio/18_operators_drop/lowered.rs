//! 18 — Operator overloading, `Drop`, `Clone` vs `Copy`, ordering and equality.

use std::ops::{Add, Mul};

/// Operator traits are std traits: implement them like any other (nested here, D45).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec2 {
    x: f64,
    y: f64,
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f64) -> Vec2 {
        Vec2 {
            x: self.x * k,
            y: self.y * k,
        }
    }
}

/// `Drop` runs when a value goes out of scope; the order is deterministic (reverse declaration).
struct Guard {
    label: &'static str,
}

impl Drop for Guard {
    fn drop(&mut self) {
        ::std::println!("drop {}", self.label);
    }
}

/// A type that is `Clone` but not `Copy`: moving it is a move; copying it is `.clone()`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Tag {
    name: String,
}

fn main() {
    let a = Vec2 { x: 1.0, y: 2.0 };
    let b = Vec2 { x: 0.5, y: 0.5 };
    ::std::println!("{:?} {:?}", a + b, (a + b) * 2.0);
    ::std::println!("{}", a == b);

    let early = Guard { label: "early" };
    let late = Guard { label: "late" };
    let _ = (early.label, late.label);
    let inner = Guard { label: "inner" };
    drop(inner);
    ::std::println!("after explicit drop");

    let mut tags = Vec::from([
        Tag {
            name: String::from("b"),
        },
        Tag {
            name: String::from("a"),
        },
    ]);
    tags.sort();
    let first = tags[0].clone();
    ::std::println!("{first:?} {}", tags.iter().max().unwrap().name);
}
