//! 10 — Errors: `throws`, `throw`, `?`, `From` conversions, a crate default error,
//! verbatim `Result` aliases, and what the tail lowering does.

use std::fmt;

/// An application error type. Nothing about it is special: it is a Rust enum.
#[derive(Debug)]
pub enum AppError {
    Parse(std::num::ParseIntError),
    Io(std::io::Error),
    Empty,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Parse(e) => write!(f, "bad number: {e}"),
            AppError::Io(e) => write!(f, "io: {e}"),
            AppError::Empty => write!(f, "empty input"),
        }
    }
}

impl std::error::Error for AppError {}

/// `From` impls make `?` convert. Implementing a *foreign* trait means writing the modes the
/// trait declares — here `take`, because `From::from` takes its argument by value (§15.4).
impl From<std::num::ParseIntError> for AppError {
    fn from(e: std::num::ParseIntError) -> Self {
        Self::Parse(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// With a default declared, private functions may write bare `throws` (D24).
/// `-> T throws E` is `Result<T, E>`. `return v` and the tail expression are wrapped in `Ok`;
/// `throw e` is `return Err(e)`; `?` propagates with `From` conversion.
fn parse_total(input: &str) -> ::core::result::Result<u64, AppError> {
    if input.trim().is_empty() {
        return ::core::result::Result::Err(AppError::Empty);
    }
    let mut total: u64 = 0;
    for part in input.split(',') {
        let n: u64 = part.trim().parse()?;
        total += n;
    }
    ::core::result::Result::Ok(total)
}

/// A tail expression that is *already* a Result ends in `?`. It does not lower to `Ok(e?)`
/// (which would copy the error payload); it lowers to a match through `__uredo_owned` that
/// is code-identical to returning the Result directly (D37).
fn read_total(path: &str) -> ::core::result::Result<u64, AppError> {
    let text = std::fs::read_to_string(path)?;
    match __uredo_owned(parse_total(&text)) {
        ::core::result::Result::Ok(__uredo_v) => ::core::result::Result::Ok(__uredo_v),
        ::core::result::Result::Err(__uredo_x) => {
            ::core::result::Result::Err(::core::convert::From::from(__uredo_x))
        }
    }
}

/// `pub` functions always name their error type (D24).
pub fn total_or_zero(input: &str) -> ::core::result::Result<u64, AppError> {
    ::core::result::Result::Ok(match parse_total(input) {
        Ok(n) => n,
        Err(AppError::Empty) => 0,
        Err(e) => return ::core::result::Result::Err(e),
    })
}

/// A verbatim `Result` type is plain Rust: no implicit `Ok`, `?` still works (D33).
fn io_probe(path: &str) -> std::io::Result<u64> {
    let meta = std::fs::metadata(path)?;
    Ok(meta.len())
}

fn main() -> ::core::result::Result<(), AppError> {
    ::std::println!("{}", parse_total("1, 2, 3")?);
    ::std::println!("{}", total_or_zero("")?);
    let fallback = match parse_total("x") {
        Ok(n) => n,
        Err(e) => {
            ::std::eprintln!("{e}");
            0
        }
    };
    ::std::println!("{fallback}");
    ::std::println!("{:?}", io_probe("/definitely/missing").is_err());
    let _ = read_total;
    ::core::result::Result::Ok(())
}

#[inline(always)]
fn __uredo_owned<T, E>(r: ::core::result::Result<T, E>) -> ::core::result::Result<T, E> {
    r
}
