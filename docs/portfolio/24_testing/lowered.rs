//! 24 — Testing and documentation: `@test`, `@cfg(test)`, `mod tests:`, `assert*!`,
//! `should_panic`, doc comments, and a `throws` test.

/// The function under test. `##` doc comments become `///` and feed `uredo doc`.
/// Parses "key=value" pairs separated by `;`. Blank parts are skipped.
pub fn parse_pairs(input: &str) -> ::core::result::Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for raw in input.split(';') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part
            .split_once('=')
            .ok_or(::std::format!("missing '=' in {part:?}"))?;
        out.push((k.trim().to_string(), v.trim().to_string()));
    }
    ::core::result::Result::Ok(out)
}

/// A function that panics on bad input, for the `should_panic` test.
pub fn checked_div(a: i32, b: i32) -> i32 {
    if b == 0 {
        ::std::panic!("division by zero");
    }
    a / b
}

fn main() {
    ::std::println!("{:?}", parse_pairs("a=1; b = 2"));
    ::std::println!("{}", checked_div(7, 2));
}

/// Tests live in an inline module compiled only under `cfg(test)` (D35); `use super::*` is
/// written, never implied.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pairs() {
        let pairs = parse_pairs("a=1; b = 2").unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[1], (String::from("b"), String::from("2")));
    }

    #[test]
    fn rejects_missing_equals() {
        let err = parse_pairs("a=1;oops").unwrap_err();
        assert!(err.contains("oops"), "unexpected message: {err}");
    }

    /// A test may itself be `throws`: `?` inside it fails the test on `Err`.
    #[test]
    fn throws_test() -> ::core::result::Result<(), String> {
        let pairs = parse_pairs("x=y")?;
        assert_eq!(pairs[0].0, "x");
        ::core::result::Result::Ok(())
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn division_by_zero_panics() {
        let _ = checked_div(1, 0);
    }
}
