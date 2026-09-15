//! 15 — Lifetimes: when elision does the work, when you write `'a`, and references in structs.

/// Elision: one borrowed parameter, so the returned `str` borrows from it (§9.7). Nothing written.
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

/// Two borrowed parameters and a borrowed return: Rust cannot guess, so the lifetime is
/// written in Rust syntax (rule P6). Uredo never invents a runtime mechanism to avoid this.
fn longer<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

/// A struct holding a reference must name the lifetime; Uredo never adds one (§9.7).
#[derive(Debug)]
struct Excerpt<'a> {
    text: &'a str,
    line: usize,
}

impl<'a> Excerpt<'a> {
    /// Methods on it: the receiver's lifetime covers the returned borrow (elision, Rust's rules).
    fn first_line(&self) -> &str {
        self.text.lines().next().unwrap_or("")
    }
}

/// `'static`: data that lives for the whole program, e.g. string literals.
fn motto() -> &'static str {
    "borrow, don't copy"
}

fn main() {
    let text = String::from("hello world\nsecond line");
    ::std::println!("{}", first_word(&text));
    ::std::println!("{}", longer("ab", "abc"));

    let ex = Excerpt {
        text: text.as_str(),
        line: 1,
    };
    ::std::println!("{ex:?} / line {} / {}", ex.line, ex.first_line());

    let mut owner = String::from("abc");
    let view = &owner;
    ::std::println!("{view}");
    owner.push('d');
    ::std::println!("{owner} {}", motto());
}
