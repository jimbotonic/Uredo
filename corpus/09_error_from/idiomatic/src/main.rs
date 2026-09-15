//! 9. Error propagation: an enum error type with `From` impls, `?` through layers.

use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
enum ConfigError {
    Parse(ParseIntError),
    Missing(String),
    Range { key: String, value: i64 },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Parse(e) => write!(f, "bad number: {e}"),
            ConfigError::Missing(key) => write!(f, "missing key {key:?}"),
            ConfigError::Range { key, value } => write!(f, "{key} out of range: {value}"),
        }
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(e: ParseIntError) -> Self {
        ConfigError::Parse(e)
    }
}

fn lookup(pairs: &[(&str, &str)], key: &str) -> Result<String, ConfigError> {
    for (k, v) in pairs {
        if *k == key {
            return Ok(v.to_string());
        }
    }
    Err(ConfigError::Missing(key.to_string()))
}

fn port(pairs: &[(&str, &str)]) -> Result<u16, ConfigError> {
    let raw = lookup(pairs, "port")?;
    let value: i64 = raw.trim().parse()?;
    if value < 1 || value > 65535 {
        return Err(ConfigError::Range {
            key: String::from("port"),
            value,
        });
    }
    Ok(value as u16)
}

fn main() {
    let configs = [
        [("host", "localhost"), ("port", "8080")],
        [("host", "localhost"), ("port", "eighty")],
        [("host", "localhost"), ("port", "70000")],
        [("host", "localhost"), ("timeout", "3")],
    ];
    for pairs in &configs {
        match port(pairs) {
            Ok(p) => println!("port {p}"),
            Err(e) => println!("error: {e}"),
        }
    }
}
