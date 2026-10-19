//! The standards server (`dev.thmsn.standards.server`): the embedded content over HTTP.
//!
//! [`app`] is the whole mount map; `main.rs` only loads config, starts telemetry and serves
//! it. Mount map (L6):
//!
//! ```text
//! /api/v1/*         REST mirror (rest.rs), poem-openapi; public, read-only
//! /manifest.json    the manifest the cli syncs against (same as /api/v1/manifest)
//! /t/{path..}       a raw template, text/plain
//! /openapi.json     the emitted contract (committed as api/openapi.json)
//! /api/docs         Scalar (not /docs: the HTML site owns /d/)
//! /health           liveness
//! /version          full libbuildinfo
//! /api/metrics      OpenMetrics, standards_* series
//! /install          302 to the registry's cli install script
//! /mcp              STD-16, mcp.rs
//! /, /d/{id..}      STD-17, web.rs
//! ```
//!
//! Content routes (`/api/v1/*`, `/manifest.json`, `/t/*`) carry `ETag: "<commit>"` and a
//! short max-age (caching.rs). Logs carry the path, never the query string (request.rs).

// #[handler] and #[OpenApi] methods are async by contract, awaiting or not.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

pub mod caching;
pub mod config;
pub mod dto;
pub mod error;
pub mod mcp;
pub mod metrics;
pub mod paths;
pub mod request;
pub mod rest;
pub mod state;
pub mod web;

use std::sync::Arc;

use poem::http::StatusCode;
use poem::http::header::LOCATION;
use poem::web::{Data, Json, Path};
use poem::{Endpoint, EndpointExt as _, Response, Route, get, handler};

pub use crate::config::Config;
pub use crate::state::AppState;

/// Where `GET /install` sends the caller: the registry's install script for the cli.
pub const INSTALL_REDIRECT: &str = "https://apps.dev.thmsn.dev/install/dev.thmsn.standards";

const JSON: &str = "application/json; charset=utf-8";
const TEXT: &str = "text/plain; charset=utf-8";

#[handler]
fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// The derived version, the content commit and the full build metadata (`null` when the
/// build had no repository to read).
#[handler]
fn version(Data(state): Data<&AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": state.version,
        "commit": state.engine.commit(),
        "build_info": standards_core::build_info(),
    }))
}

#[handler]
fn install() -> Response {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, INSTALL_REDIRECT)
        .finish()
}

#[handler]
fn scrape(Data(state): Data<&AppState>) -> Response {
    Response::builder()
        .content_type(metrics::OPENMETRICS)
        .body(state.metrics.render())
}

/// The core manifest, serialized as the cli's `standards-core` types read it.
#[handler]
fn manifest_json(Data(state): Data<&AppState>) -> Response {
    Response::builder()
        .content_type(JSON)
        .body(serde_json::to_string(state.manifest.as_ref()).unwrap_or_default())
}

/// A template's raw bytes, placeholders and all.
#[handler]
fn raw_template(Data(state): Data<&AppState>, Path(path): Path<String>) -> poem::Result<Response> {
    let template = state
        .engine
        .template(&path)
        .map_err(|e| error::to_poem(&e))?;
    Ok(Response::builder()
        .content_type(TEXT)
        .body(template.body.clone()))
}

/// Each contract operation's route template (`/api/v1/docs/{id}`), from the spec, so the
/// request layer labels a route the way the contract names it.
fn route_templates() -> request::RouteTemplates {
    let spec: serde_json::Value = serde_json::from_str(&rest::spec()).unwrap_or_default();
    let mut out = request::RouteTemplates::new();
    for (path, ops) in spec["paths"].as_object().into_iter().flatten() {
        for op in ops.as_object().into_iter().flatten().map(|(_, op)| op) {
            if let Some(id) = op["operationId"].as_str() {
                out.insert(id.to_owned(), format!("{}{path}", rest::API_MOUNT));
            }
        }
    }
    out
}

/// The whole server.
pub fn app(state: AppState) -> impl Endpoint<Output = Response> {
    let api = rest::service();
    let spec = api.spec_endpoint();
    let scalar = api.scalar();
    let cache = || caching::Immutable::new(state.etag.clone());

    let route = Route::new()
        .nest(rest::API_MOUNT, api.with(cache()))
        .at("/manifest.json", get(manifest_json).with(cache()))
        .at("/t/*path", get(raw_template).with(cache()))
        .at("/openapi.json", spec)
        .nest("/api/docs", scalar)
        .at("/health", get(health))
        .at("/version", get(version))
        .at("/api/metrics", get(scrape))
        .at("/install", get(install));
    let route = mcp::mount(route, &state); // STD-16: /mcp, in mcp.rs
    let route = web::mount(route, &state); // STD-17: / and /d/{id}, in web.rs

    let metrics = state.metrics.clone();
    let routes = Arc::new(route_templates());
    route
        .around(paths::rewrite)
        .data(state)
        .map_to_response()
        // Inside the request layer, so a panic is a logged 500 with a trace id.
        .with(poem::middleware::CatchPanic::new().with_handler(request::panic_response))
        .around(move |ep, req| request::serve(ep, req, metrics.clone(), routes.clone()))
}

#[cfg(test)]
mod tests;
