//! Query parameters, parsed without allocating.

use crate::error::Error;

/// What `GET /items` accepts. `'a` is the lifetime of the raw query string.
#[derive(Debug, PartialEq)]
pub struct Query<'a> {
    pub done: Option<bool>,
    pub label: Option<&'a str>,
    pub contains: Option<&'a str>,
    pub limit: usize,
    pub offset: usize,
}

/// The ceiling on a page.
pub const MAX_LIMIT: usize = 200;

impl<'a> Query<'a> {
    pub fn empty() -> Query<'a> {
        Query { done: None, label: None, contains: None, limit: MAX_LIMIT, offset: 0 }
    }

    /// An unknown key is an error rather than a shrug: a service that silently ignores
    /// `limti=20` returns the wrong page and says nothing.
    pub fn parse(raw: &'a str) -> Result<Query<'a>, Error> {
        let mut q = Query::empty();
        for pair in raw.split('&').filter(|s| !s.is_empty()) {
            let Some((key, value)) = pair.split_once('=') else {
                return Err(Error::BadRequest(format!("query parameter {pair:?} has no value")));
            };
            match key {
                "done" => q.done = Some(parse_bool(value)?),
                "label" => q.label = Some(value),
                "contains" => q.contains = Some(value),
                "limit" => q.limit = parse_usize(value, "limit")?.min(MAX_LIMIT),
                "offset" => q.offset = parse_usize(value, "offset")?,
                _ => return Err(Error::BadRequest(format!("unknown query parameter {key:?}"))),
            }
        }
        Ok(q)
    }
}

fn parse_bool(value: &str) -> Result<bool, Error> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::BadRequest(format!("expected true or false, got {value:?}"))),
    }
}

fn parse_usize(value: &str, name: &str) -> Result<usize, Error> {
    value
        .parse()
        .map_err(|_e| Error::BadRequest(format!("{name} must be a whole number, got {value:?}")))
}
