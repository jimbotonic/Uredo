//! What goes on the wire, and how it is negotiated.
//!
//! Two choices, independent of each other, both opt-in: the shape (keyed JSON, or the same
//! values positionally with the field names dropped) and the encoding (identity or brotli). A
//! service with a fixed type set already knows the field order, so repeating the names in every
//! element of every list is paying for a schema the client also has.

use serde_json::Value;

use crate::field::Map;
use crate::model::{Health, Item};

/// Which shape the client asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wire {
    Keyed,
    Positional,
}

/// Which content coding the client will accept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coding {
    Identity,
    Brotli,
}

/// The media type that asks for the positional shape. A vendor type, because the compact form is
/// this service's schema rather than an interchange one.
pub const COMPACT: &str = "application/vnd.restdemo.compact+json";

impl Wire {
    pub fn of(accept: Option<&str>) -> Wire {
        match accept {
            Some(a) if a.contains(COMPACT) => Wire::Positional,
            _ => Wire::Keyed,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Wire::Keyed => "application/json",
            Wire::Positional => COMPACT,
        }
    }
}

impl Coding {
    pub fn of(accept_encoding: Option<&str>) -> Coding {
        match accept_encoding {
            Some(a) if a.contains("br") => Coding::Brotli,
            _ => Coding::Identity,
        }
    }
}

/// A value that can drop its field names.
pub trait Positional {
    fn positional(&self) -> Value;
}

impl Positional for Item {
    fn positional(&self) -> Value {
        Value::Array(vec![
            self.id.into(),
            self.priority.into(),
            self.weight.into(),
            Value::String(self.name.clone()),
            self.done.into(),
            optional_string(self.tag.as_ref()),
            strings(&self.labels),
            dictionary(&self.meta),
        ])
    }
}

impl Positional for Health {
    fn positional(&self) -> Value {
        Value::Array(vec![Value::String(self.status.clone()), self.items.into()])
    }
}

impl<T: Positional> Positional for Vec<T> {
    fn positional(&self) -> Value {
        Value::Array(self.iter().map(|x| x.positional()).collect())
    }
}

fn optional_string(v: Option<&String>) -> Value {
    match v {
        Some(s) => Value::String(s.clone()),
        None => Value::Null,
    }
}

fn strings(v: &[String]) -> Value {
    Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())
}

fn dictionary(v: &Map<String>) -> Value {
    Value::Object(v.iter().map(|(k, s)| (k.clone(), Value::String(s.clone()))).collect())
}
