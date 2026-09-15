//! 01 — Hello: the smallest Uredo program.
//!
//! `##!` is a module doc comment (Rust `//!`), `##` documents the next item (Rust `///`),
//! and `#` is an ordinary comment. Attributes use `@` because `#` is taken.

/// The entry point. No `->` type means the function returns unit.
/// A colon ends the header; the body is the indented block. No braces, no semicolons.
fn main() {
    let name = "world";
    ::std::println!("Hello, {name}!");

    ::std::println!("{} + {} = {}", 2, 3, 2 + 3);

    let greeting: String = ::std::format!("Hello again, {name}");
    ::std::println!("{}", greeting);

    let pair = (1, "two");
    ::std::println!("{pair:?}");
}
