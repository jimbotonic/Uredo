//! The same service, with axum supplying the HTTP edge.
//!
//! Everything below the edge — the store, the model, the query grammar, the wire shapes, the
//! ETag, the error surface — is `restdemo_idiomatic`, imported rather than rewritten. What this
//! crate replaces is exactly the three files the hand-rolled twin spends on HTTP: the accept
//! loop and drain in its `lib.rs`, the `match` in its `router.rs`, and the dispatch in its
//! `wire.rs`. Responses are still built by the twin's `handler`, so both binaries put the same
//! bytes on the wire by construction and not by inspection.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use bytes::Bytes;
use tokio::net::TcpListener;
use tokio::sync::Notify;

use restdemo_idiomatic::error::Error;
use restdemo_idiomatic::handler::{MAX_BODY, Reply, Wants, no_content, raw, value_out, value_out_cached};
use restdemo_idiomatic::model::{ItemPatch, NewItem};
use restdemo_idiomatic::query::Query;
use restdemo_idiomatic::repr::{Coding, Wire};
use restdemo_idiomatic::store::Store;

/// The routing table.
pub fn app(state: Arc<Store>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/items", get(list).post(create))
        .route("/items/{id}", get(read).patch(patch).delete(delete))
        // The twin answers an unrouted path with its own JSON error body; so does this.
        .fallback(missing)
        // The twin reads a body with a ceiling by hand. axum has the ceiling as a layer, and
        // rejects over it before the handler runs, which is the better place for it.
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

/// Serve until the process is killed.
pub async fn serve(listener: TcpListener, state: Arc<Store>) -> Result<(), Error> {
    serve_until(listener, state, Arc::new(Notify::new())).await
}

/// Serve until `shutdown` is notified, then stop accepting and let in-flight work finish.
///
/// The twin hand-rolls this: a `select!` in the accept loop, an `AtomicUsize` incremented per
/// spawned connection, and a poll-until-zero drain with a five-second deadline. Here it is an
/// argument, and the draining is axum's.
pub async fn serve_until(listener: TcpListener, state: Arc<Store>, shutdown: Arc<Notify>) -> Result<(), Error> {
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async move { shutdown.notified().await })
        .await?;
    Ok(())
}

/// A service error, on its way to becoming a response.
///
/// axum wants a rejection type that knows how to render itself. Ours already knows its status
/// and its message, so this is the adapter and not a second error surface.
pub struct Fault(Error);

impl From<Error> for Fault {
    fn from(e: Error) -> Fault {
        Fault(e)
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        let body = format!("{{\"error\":{:?}}}", self.0.message());
        into(raw(self.0.status(), body.as_bytes()))
    }
}

/// The shared representation code builds hyper responses; axum takes the same bytes.
fn into(r: Reply) -> Response {
    r.map(Body::new)
}

type Out = Result<Response, Fault>;

async fn health(State(s): State<Arc<Store>>) -> Out {
    Ok(into(value_out(200, || Ok(s.health()))?))
}

async fn list(State(s): State<Arc<Store>>, RawQuery(q): RawQuery, h: HeaderMap) -> Out {
    let filter = Query::parse(q.as_deref().unwrap_or(""))?;
    Ok(into(value_out_cached(200, &wants(&h), || Ok(s.select(&filter)))?))
}

async fn read(State(s): State<Arc<Store>>, Path(id): Path<String>, h: HeaderMap) -> Out {
    let id = number(&id)?;
    Ok(into(value_out_cached(200, &wants(&h), || s.get(id).ok_or(Error::NotFound))?))
}

async fn create(State(s): State<Arc<Store>>, body: Bytes) -> Out {
    let new: NewItem = serde_json::from_slice(&body).map_err(Error::from)?;
    new.validate()?;
    let item = s.create(new)?;
    Ok(into(raw(201, &serde_json::to_vec(&item).map_err(Error::from)?)))
}

async fn patch(State(s): State<Arc<Store>>, Path(id): Path<String>, body: Bytes) -> Out {
    let id = number(&id)?;
    let change: ItemPatch = serde_json::from_slice(&body).map_err(Error::from)?;
    let item = s.patch(id, change).ok_or(Error::NotFound)?;
    Ok(into(raw(200, &serde_json::to_vec(&item).map_err(Error::from)?)))
}

async fn delete(State(s): State<Arc<Store>>, Path(id): Path<String>) -> Out {
    let id = number(&id)?;
    Ok(into(no_content(|| if s.delete(id) { Ok(()) } else { Err(Error::NotFound) })?))
}

async fn missing() -> Fault {
    Fault(Error::NotFound)
}

/// `/items/abc` is a path this service does not have, which is a 404 and not a 400.
///
/// `Path<u64>` would be the shorter extractor and would answer 400, because to axum a segment
/// that will not parse is a malformed request. The twin's router returns `None` from the same
/// case and the dispatcher turns that into `NotFound`. Matching the twin costs this function
/// and one line in each of three handlers.
fn number(segment: &str) -> Result<u64, Error> {
    segment.parse().map_err(|_| Error::NotFound)
}

/// What the client asked for: the shape, the coding, and the tag it already holds.
fn wants(h: &HeaderMap) -> Wants {
    let text = |name: &str| h.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    Wants {
        wire: Wire::of(text("accept").as_deref()),
        coding: Coding::of(text("accept-encoding").as_deref()),
        if_none_match: text("if-none-match"),
    }
}
