//! 08 — Enums and pattern matching: algebraic data, exhaustiveness, guards,
//! `if let`, `while let`, `let-else`.

#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Dot,
    Circle(f64),
    Rect { w: f64, h: f64 },
}

impl Shape {
    /// Methods and trait impls may live in the enum body too (D45).
    fn area(&self) -> f64 {
        match *self {
            Shape::Dot => 0.0,
            Shape::Circle(r) => std::f64::consts::PI * r * r,
            Shape::Rect { w, h } => w * h,
        }
    }
}

#[derive(Debug)]
enum Command {
    Quit,
    Say(String),
    Move { dx: i32, dy: i32 },
}

/// `match` must be exhaustive; `_` is the catch-all. Guards are `pattern if cond:`.
fn run(cmd: &Command) -> String {
    match cmd {
        Command::Quit => String::from("bye"),
        Command::Say(text) if text.is_empty() => String::from("(silence)"),
        Command::Say(text) => ::std::format!("says {text}"),
        Command::Move { dx: 0, dy: 0 } => String::from("stays"),
        Command::Move { dx, dy } => ::std::format!("moves by ({dx}, {dy})"),
    }
}

fn main() {
    let shapes = Vec::from([
        Shape::Dot,
        Shape::Circle(1.0),
        Shape::Rect { w: 2.0, h: 3.0 },
    ]);
    for s in &shapes {
        ::std::println!("{s:?} -> {:.2}", s.area());
    }

    for c in [
        Command::Quit,
        Command::Say(String::new()),
        Command::Move { dx: 1, dy: -1 },
    ] {
        ::std::println!("{}", run(&c));
    }

    let maybe = Some(Shape::Circle(2.0));
    if let Some(Shape::Circle(r)) = &maybe {
        ::std::println!("radius {r}");
    }

    let mut stack = Vec::from([1, 2, 3]);
    while let Some(top) = stack.pop() {
        ::std::println!("pop {top}");
    }

    let Some(first) = shapes.first() else {
        return;
    };
    ::std::println!("first is {first:?}");

    ::std::println!("{}", matches!(shapes[1], Shape::Circle(r) if r > 0.5));
}
