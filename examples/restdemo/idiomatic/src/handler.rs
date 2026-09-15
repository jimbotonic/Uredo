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
use crate::repr::{Coding, Positional, Wire};

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

/// What the client asked for, gathered once per request: the shape it will read, the codings it
/// will accept, and the tag it already holds.
pub struct Wants {
    pub wire: Wire,
    pub coding: Coding,
    pub if_none_match: Option<String>,
}

/// Below this a brotli frame costs more than it saves — measured on this service's own payloads,
/// where a 56-byte positional `Item` does not shrink at all.
pub const COMPRESS_ABOVE: usize = 256;

/// The same, but the client may already hold it, and may want it in a different shape or coding.
///
/// The tag is computed over the bytes actually sent, so a keyed body, a positional body and a
/// brotli frame each get their own — which is what `Vary` then tells a cache to key on.
pub fn value_out_cached<Out: Serialize + Positional>(
    status: u16,
    wants: &Wants,
    f: impl FnOnce() -> Result<Out, Error>,
) -> Result<Reply, Error> {
    let value = f()?;
    let shaped = match wants.wire {
        Wire::Keyed => serde_json::to_vec(&value)?,
        Wire::Positional => serde_json::to_vec(&value.positional())?,
    };
    let (body, coding) = if wants.coding == Coding::Brotli && shaped.len() > COMPRESS_ABOVE {
        (compress(&shaped)?, Coding::Brotli)
    } else {
        (shaped, Coding::Identity)
    };
    let tag = etag::of(&body);
    if let Some(offered) = wants.if_none_match.as_deref() {
        if etag::matches(offered, &tag) {
            return Ok(not_modified(&tag));
        }
    }
    Ok(representation(status, &body, &tag, wants.wire, coding))
}

/// Brotli at quality 5, which is where the size curve flattens for bodies of this size.
fn compress(body: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Write;
    let mut out = Vec::new();
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
        w.write_all(body).map_err(|_| Error::Internal("brotli".to_string()))?;
    }
    Ok(out)
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

/// A representation, its tag, and what a cache must key on to keep them apart.
fn representation(status: u16, body: &[u8], tag: &str, wire: Wire, coding: Coding) -> Reply {
    let mut b = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", wire.content_type())
        .header("etag", tag)
        .header("vary", "accept, accept-encoding");
    if coding == Coding::Brotli {
        b = b.header("content-encoding", "br");
    }
    b.body(Full::new(Bytes::copy_from_slice(body))).unwrap()
}

/// No body at all, which is the point of the whole exchange.
fn not_modified(tag: &str) -> Reply {
    Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header("etag", tag)
        .header("vary", "accept, accept-encoding")
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
