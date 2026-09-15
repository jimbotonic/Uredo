//! Connection handling and dispatch.

use std::sync::Arc;

use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use crate::error::Error;
use crate::handler::{Reply, Wants, body_in, no_content, raw, value_out, value_out_cached};
use crate::model::{Item, ItemPatch, NewItem};
use crate::query::Query;
use crate::repr::{Coding, Wire};
use crate::router::{Route, resolve};
use crate::store::Store;

/// Serve one connection.
pub async fn connection(stream: TcpStream, state: Arc<Store>) -> Result<(), Error> {
    let io = TokioIo::new(stream);
    let service = service_fn(move |req: Request<Incoming>| {
        let state = Arc::clone(&state);
        async move { respond(req, state).await }
    });
    hyper::server::conn::http1::Builder::new()
        .serve_connection(io, service)
        .await?;
    Ok(())
}

/// One request to one response. Never fails: a service error becomes a status here.
pub async fn respond(
    req: Request<Incoming>,
    state: Arc<Store>,
) -> Result<Reply, Error> {
    Ok(match dispatch(req, state).await {
        Ok(r) => r,
        Err(e) => raw(e.status(), format!("{{\"error\":{:?}}}", e.message()).as_bytes()),
    })
}

/// The whole routing table. Adding an endpoint is one arm here and one function it calls.
async fn dispatch(
    req: Request<Incoming>,
    state: Arc<Store>,
) -> Result<Reply, Error> {
    let method = req.method().as_str().to_owned();
    let path = req.uri().path().to_owned();
    let Some(found) = resolve(&method, &path) else {
        return Err(Error::NotFound);
    };
    let raw_query = req.uri().query().unwrap_or("");
    let wants = Wants {
        wire: Wire::of(header(&req, "accept")),
        coding: Coding::of(header(&req, "accept-encoding")),
        if_none_match: header(&req, "if-none-match").map(str::to_string),
    };

    match found {
        Route::Health => value_out(200, || Ok(state.health())),
        Route::ListItems => {
            let filter = Query::parse(raw_query)?;
            value_out_cached(200, &wants, || Ok(state.select(&filter)))
        }
        Route::GetItem(id) => {
            value_out_cached(200, &wants, || state.get(id).ok_or(Error::NotFound))
        }
        Route::DeleteItem(id) => no_content(|| {
            if state.delete(id) { Ok(()) } else { Err(Error::NotFound) }
        }),
        Route::CreateItem => body_in(req, 201, |new: NewItem| create(&state, new)).await,
        Route::PatchItem(id) => {
            body_in(req, 200, |patch: ItemPatch| {
                state.patch(id, patch).ok_or(Error::NotFound)
            })
            .await
        }
    }
}

/// One request header, as a borrowed string.
fn header<'a>(req: &'a Request<Incoming>, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

/// The one endpoint with logic of its own.
fn create(state: &Arc<Store>, new: NewItem) -> Result<Item, Error> {
    new.validate()?;
    state.create(new)
}
