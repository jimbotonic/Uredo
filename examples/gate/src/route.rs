//! Gate item 1 (§34): a generic Rust callee whose meaning depends on whether the argument is
//! owned or borrowed. Uredo passes arguments to Rust items verbatim (§10.3), so `route(n)`
//! and `route(&n)` written in Uredo mean exactly what they say.

pub trait Mode {
    fn name() -> &'static str;
}

impl Mode for u8 {
    fn name() -> &'static str {
        "owned"
    }
}

impl Mode for &u8 {
    fn name() -> &'static str {
        "borrowed"
    }
}

pub fn route<T: Mode>(_value: T) -> &'static str {
    T::name()
}
