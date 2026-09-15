//! 14 — Traits: definitions with default methods, implementations, static dispatch,
//! trait objects (`dyn`, boxed and borrowed), `impl Trait` returns, std traits.

/// A trait declaration. Signatures have no body, so receivers and types are written out.
trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> String;
    /// A default method: implementors may override it.
    fn describe(&self) -> String {
        ::std::format!("{} with area {:.2}", self.name(), self.area())
    }
}

#[derive(Debug, Clone, Copy)]
struct Circle {
    r: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.r * self.r
    }
    fn name(&self) -> String {
        String::from("circle")
    }
}

#[derive(Debug, Clone, Copy)]
struct Square {
    side: f64,
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }
    fn name(&self) -> String {
        String::from("square")
    }
    fn describe(&self) -> String {
        ::std::format!("a {}-unit square", self.side)
    }
}

/// Static dispatch: monomorphised per concrete type. `T` passes by value (P8): a non-Copy
/// argument is moved, so the shapes below are `@derive(Copy)` or are not used again.
fn print_static<T: Shape>(shape: T) {
    ::std::println!("static: {}", shape.describe());
}

/// Dynamic dispatch: `dyn Trait` is written, a vtable is used, nothing is hidden.
fn print_dyn(shape: &dyn Shape) {
    ::std::println!("dyn: {}", shape.describe());
}

/// Returning `impl Trait`: the concrete type is hidden from the caller, not from the compiler.
fn unit_square() -> impl Shape {
    Square { side: 1.0 }
}

/// Standard traits work the same way; `Default` and `PartialOrd` are derived or written.
#[derive(Debug, Default, PartialEq, PartialOrd)]
struct Version {
    major: u32,
    minor: u32,
}

impl std::str::FromStr for Version {
    type Err = String;
    fn from_str(s: &str) -> ::core::result::Result<Self, String> {
        let (a, b) = s.split_once('.').ok_or(String::from("expected a.b"))?;
        let major = a.parse::<u32>().map_err(|e| e.to_string())?;
        let minor = b.parse::<u32>().map_err(|e| e.to_string())?;
        ::core::result::Result::Ok(Self { major, minor })
    }
}

fn main() {
    let c = Circle { r: 1.0 };
    let s = Square { side: 2.0 };
    print_static(c);
    print_static(s);
    print_dyn(&c);
    let shapes: Vec<Box<dyn Shape>> = Vec::from([Box::new(c) as Box<dyn Shape>, Box::new(s)]);
    for sh in &shapes {
        ::std::println!("{}", sh.describe());
    }
    ::std::println!("{}", unit_square().describe());

    let v: Version = "1.4".parse().unwrap();
    ::std::println!("{v:?} newer than default: {}", v > Version::default());
    ::std::println!("{:?}", "oops".parse::<Version>());
}
