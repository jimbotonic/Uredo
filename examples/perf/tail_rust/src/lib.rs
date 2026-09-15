//! Hand-written twin of `../../tail_ure/src/lib.ure` (§36 tail-`?` fixture). `archive` is the direct
//! return that the same-type tail `?` must match; `convert` is the `Ok(e?)` form that the converting
//! tail must not be worse than.

#[derive(Debug)]
#[allow(dead_code)] // the payload exists to be 32 bytes wide
pub struct BigError {
    pub code: u64,
    pub a: u64,
    pub b: u64,
    pub c: u64,
}

#[derive(Debug)]
#[allow(dead_code)] // the payload is carried, never read here
pub struct OuterError {
    pub inner: BigError,
}

impl From<BigError> for OuterError {
    fn from(e: BigError) -> Self {
        OuterError { inner: e }
    }
}

const _: () = assert!(::core::mem::size_of::<BigError>() == 32);

#[inline(never)]
pub fn inner(n: u64) -> Result<u64, BigError> {
    if n == 0 {
        return Err(BigError { code: 1, a: 2, b: 3, c: 4 });
    }
    Ok(n * 2)
}

/// The direct return: no `?`, no wrapping.
pub fn archive(n: u64) -> Result<u64, BigError> {
    inner(n)
}

/// The form D37 replaced, kept here as the baseline the converting tail must not exceed.
pub fn convert(n: u64) -> Result<u64, OuterError> {
    Ok(inner(n)?)
}
