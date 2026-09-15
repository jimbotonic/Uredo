//! The accepted type set, as a trait rather than a comment.

use std::collections::BTreeMap;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A dictionary. `BTreeMap` rather than `HashMap`: ordered output without a sort.
pub type Map<T> = BTreeMap<String, T>;

/// Implemented for exactly the accepted types.
pub trait Field: Serialize + DeserializeOwned {}

macro_rules! field {
    ($($t:ty),* $(,)?) => { $(impl Field for $t {})* };
}

field!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, String);

impl<T: Field> Field for Vec<T> {}
impl<T: Field> Field for Map<T> {}
impl<T: Field> Field for Option<T> {}
