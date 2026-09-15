//! 1. Hello world: printing, interpolation, an owned string.

fn main() {
    let name = "world";
    println!("Hello, {name}!");
    let greeting: String = format!("Hello again, {name}");
    println!("{} ({} bytes)", greeting, greeting.len());
}
