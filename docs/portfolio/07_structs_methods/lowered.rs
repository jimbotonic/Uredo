//! 07 — Structs and methods: construction, receivers, associated functions,
//! field sugar, nested impls.

/// Fields are private to the module unless `pub`, as in Rust. `@derive` forwards to `#[derive]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    /// An *associated function* (no `self`): the constructor idiom. `Self` is the enclosing type.
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub fn square(side: f64) -> Self {
        Self::new(side, side)
    }

    /// Receivers are always written (D27): `self` is `&self`, `self: inout` is `&mut self`,
    /// `self: take` is `self` by value. Inside a method a bare field name is a *read* of
    /// `self.field` (D41); writes must spell `self.field`.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    pub fn scale(&mut self, factor: f64) {
        self.width *= factor;
        self.height *= factor;
    }

    /// Consuming receiver: the builder pattern. `var self: take` is Rust's `mut self`.
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = height;
        self
    }
}

impl std::fmt::Display for Rectangle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Tuple structs and unit structs keep Rust's spelling; a colon-less header has no body (D35).
struct Meters(f64);
struct Marker;

fn main() {
    let mut r = Rectangle::new(2.0, 3.0);
    ::std::println!("{r} area {}", r.area());
    r.scale(2.0);
    ::std::println!("{r:?}");
    let tall = Rectangle::square(1.0).with_height(5.0);
    ::std::println!(
        "{tall} == {}: {}",
        Rectangle::new(1.0, 5.0),
        tall == Rectangle::new(1.0, 5.0)
    );
    let m = Meters(3.5);
    ::std::println!("{} m", m.0);
    let _ = Marker;

    let wide = Rectangle {
        width: 10.0,
        ..tall
    };
    ::std::println!("{wide}");
}
