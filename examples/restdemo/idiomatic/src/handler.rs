//! Adding an endpoint, as small as it goes.
//!
//! An endpoint is a plain function over typed values — it never sees a request, a status or a
//! byte. These adapters supply the HTTP, and are generic so each monomorphises at its call site.

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::etag;

/// Every handler returns this.
pub type Reply = Response<Full<Bytes>>;

/// A request body is read with a fixed ceiling.
pub const MAX_BODY: usize = 64 * 1024;

/// Body in, value out — `POST`, `PATCH`.
pub async fn body_in<In: DeserializeOwned, Out: Serialize>(
    req: Request<Incoming>,
    status: u16,
    f: impl FnOnce(In) -> Result<Out, Error>,
) -> Result<Reply, Error> {
    let raw = read_body(req).await?;
    let value: In = serde_json::from_slice(&raw)?;
    encoded(status, &f(value)?)
}

/// Nothing in, value out — `GET`.
pub fn value_out<Out: Serialize>(
    status: u16,
    f: impl FnOnce() -> Result<Out, Error>,
) -> Result<Reply, Error> {
    encoded(status, &f()?)
}

/// The same, but the client may already hold it.
pub fn value_out_cached<Out: Serialize>(
    status: u16,
    if_none_match: Option<&str>,
    f: impl FnOnce() -> Result<Out, Error>,
) -> Result<Reply, Error> {
    let body = serde_json::to_vec(&f()?)?;
    let tag = etag::of(&body);
    if let Some(offered) = if_none_match {
        if etag::matches(offered, &tag) {
            return Ok(not_modified(&tag));
        }
    }
    Ok(tagged(status, &body, &tag))
}

/// Nothing in, nothing out — `DELETE`.
pub fn no_content(f: impl FnOnce() -> Result<(), Error>) -> Result<Reply, Error> {
    f()?;
    Ok(raw(204, b""))
}

pub async fn read_body(req: Request<Incoming>) -> Result<Bytes, Error> {
    let bytes = req.into_body().collect().await?.to_bytes();
    if bytes.len() > MAX_BODY {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}

fn encoded<T: Serialize>(status: u16, value: &T) -> Result<Reply, Error> {
    Ok(raw(status, &serde_json::to_vec(value)?))
}

/// A representation and its tag.
fn tagged(status: u16, body: &[u8], tag: &str) -> Reply {
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", "application/json")
        .header("etag", tag)
        .body(Full::new(Bytes::copy_from_slice(body)))
        .unwrap()
}

/// No body at all, which is the point of the whole exchange.
fn not_modified(tag: &str) -> Reply {
    Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header("etag", tag)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

/// One response, one allocation.
pub fn raw(status: u16, body: &[u8]) -> Reply {
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", "application/json")
        .body(Full::new(Bytes::copy_from_slice(body)))
        .unwrap()
}
