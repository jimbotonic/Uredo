//! 11. Generic functions and a generic struct: bounds, `where`, by-value type parameters.

use std::fmt::Display;

fn largest<T: PartialOrd + Copy>(items: &[T]) -> Option<T> {
    let mut best = *items.first()?;
    for item in items {
        if *item > best {
            best = *item;
        }
    }
    Some(best)
}

fn describe_all<T>(items: &[T], label: impl Display) -> Vec<String>
where
    T: Display,
{
    items.iter().map(|i| format!("{label}: {i}")).collect()
}

fn pair_up<A, B>(a: A, b: B) -> (A, B) {
    (a, b)
}

#[derive(Debug)]
struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }
    fn push(&mut self, item: T) {
        self.items.push(item)
    }
    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }
    fn peek(&self) -> Option<&T> {
        self.items.last()
    }
    fn len(&self) -> usize {
        self.items.len()
    }
}

fn main() {
    println!(
        "{:?} {:?}",
        largest(&[3, 9, 2]),
        largest(&Vec::<f64>::new())
    );
    for line in describe_all(&["x", "y"], "item") {
        println!("{line}");
    }
    let (n, s) = pair_up(1, String::from("one"));
    println!("{n} {s}");
    let mut stack: Stack<String> = Stack::new();
    stack.push(String::from("a"));
    stack.push(String::from("b"));
    println!("{:?} len {}", stack.peek(), stack.len());
    println!("{:?} {:?}", stack.pop(), stack.pop());
    println!("{:?}", stack.pop());
}
