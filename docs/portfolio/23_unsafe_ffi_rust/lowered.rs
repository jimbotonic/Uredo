//! 23 — The escape hatches: `unsafe:` blocks, raw pointers, `rust { }` item blocks,
//! `extern "C"` FFI, and macros. `unsafe` is never inferred (§24); raw Rust is always available.

/// An `unsafe:` block is Rust's `unsafe { }`: the boundary is explicit and auditable.
fn first_byte(bytes: &[u8]) -> ::core::option::Option<u8> {
    if bytes.is_empty() {
        return None;
    }
    let ptr = bytes.as_ptr();
    let v = unsafe { *ptr };
    Some(v)
}

/// Anything Uredo does not model yet is written as Rust inside `rust { }` (§22.2): here an
/// `extern "C"` block calling libc's `abs`, and a `#[repr(C)]` type for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CPoint {
    pub x: i32,
    pub y: i32,
}

unsafe extern "C" {
    fn abs(x: i32) -> i32;
}

pub fn c_abs(x: i32) -> i32 {
    unsafe { abs(x) }
}

/// Items from the block are ordinary Rust items: callable, but their signatures are opaque to
/// Uredo, so arguments are passed verbatim (D2).
fn manhattan(p: &CPoint) -> i32 {
    c_abs(p.x) + c_abs(p.y)
}

/// A `macro_rules!` definition is Rust source; `macro_rules! name {` switches the lexer (D40).
macro_rules! square {
    ($e:expr) => {
        $e * $e
    };
}

fn main() {
    ::std::println!("{:?} {:?}", first_byte(b"hi"), first_byte(b""));
    ::std::println!("{}", manhattan(&CPoint { x: -3, y: 4 }));
    ::std::println!("{}", square!(7));

    {
        let mut v = vec![3, 1, 2];
        v.sort_unstable();
        println!("{v:?}");
    }

    let len: usize = { std::mem::size_of::<CPoint>() };
    ::std::println!("CPoint is {len} bytes");

    let bits = 1.5f32.to_bits();
    ::std::println!("{bits:#x}");
}
