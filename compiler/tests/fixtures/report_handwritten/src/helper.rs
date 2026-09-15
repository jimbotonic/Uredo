//! Hand-written Rust with a mistake of its own: a `&str` where a `u32` is declared.
pub fn two() -> u32 {
    let _x: u32 = "not a number";
    2
}
