//! 16 — Modules and visibility: inline modules, `pub`, `pub(crate)`, `use`, re-exports.
//! (In a real project each file is a module automatically, §20.3; inline modules are D35.)

/// An inline module is `mod name:` with an indented body. Items are private unless `pub`.
mod geometry {
    pub const UNIT: f64 = 1.0;

    #[derive(Debug, Clone, Copy)]
    pub struct Point {
        pub x: f64,
        pub y: f64,
    }

    impl Point {
        pub fn origin() -> Self {
            Self { x: 0.0, y: 0.0 }
        }
        pub fn dist(&self, other: Point) -> f64 {
            ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
        }
    }

    /// `pub(crate)`: visible in this crate only. Rust's visibility forms are unchanged.
    pub(crate) fn scale(p: Point, k: f64) -> Point {
        Point {
            x: p.x * k,
            y: p.y * k,
        }
    }

    /// A nested module, and a private helper the outside cannot see.
    pub mod shapes {
        use super::Point;

        pub struct Segment {
            pub a: Point,
            pub b: Point,
        }

        impl Segment {
            pub fn length(&self) -> f64 {
                self.a.dist(self.b)
            }
        }

        fn secret() -> u8 {
            42
        }
        pub fn reveal() -> u8 {
            secret()
        }
    }
}

pub use geometry::UNIT as ONE;
/// `use` imports as in Rust, including renames and re-exports.
use geometry::{Point, shapes::Segment};

fn main() {
    let p = Point::origin();
    let q = Point { x: 3.0, y: 4.0 };
    ::std::println!("{}", p.dist(q));
    let s = Segment {
        a: p,
        b: geometry::scale(q, 2.0),
    };
    ::std::println!("{} {}", s.length(), geometry::shapes::reveal());
    ::std::println!("{ONE}");
}
