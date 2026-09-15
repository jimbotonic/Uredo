//! 12. Trait implementation: a trait with a default method, two impls, static and dynamic dispatch.

use std::fmt;

trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> String;
    fn describe(&self) -> String {
        format!("{} with area {:.2}", self.name(), self.area())
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

struct Rect {
    w: f64,
    h: f64,
}

impl Shape for Rect {
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn name(&self) -> String {
        String::from("rect")
    }
    fn describe(&self) -> String {
        format!("{}x{} rect", self.w, self.h)
    }
}

fn print_static<T: Shape>(shape: T) {
    println!("static: {}", shape.describe());
}

fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "○ r={}", self.r)
    }
}

fn main() {
    let c = Circle { r: 1.0 };
    print_static(c);
    print_static(Rect { w: 2.0, h: 3.0 });
    let shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(c) as Box<dyn Shape>,
        Box::new(Rect { w: 1.0, h: 1.0 }),
    ];
    println!("total {:.2}", total_area(&shapes));
    println!("{c}");
}
