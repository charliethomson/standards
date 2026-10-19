// Domain error + HTTP mapping. Split across two crates:
//   - the `Error` enum + `Result` alias live in `core` (HTTP-agnostic),
//   - `to_poem` lives in `api` (the only crate that knows about HTTP).
// See standards/docs/error-handling.md.

// ─────────────────────────── core/src/error.rs ───────────────────────────
// Categorised domain error. Variants map cleanly onto HTTP status codes in the
// `api` crate without core needing to know where the failure originated.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{entity} not found")]
    NotFound { entity: &'static str },
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }
}

// ─────────────────────────── api/src/error.rs ────────────────────────────
// The ONLY place domain errors meet HTTP. Handlers do `.map_err(to_poem)?`.
/*
use poem::http::StatusCode;
use core_crate::Error;

use crate::request; // templates/rust/request_layer.rs

// A client error's message is its Display (written for the caller). A server
// error's is fixed text: its Display carries SQL, paths or an upstream's body,
// which go onto the request span instead. No log here: the request layer's one
// `http completed` line logs a 5xx, so it is logged once.
pub fn to_poem(err: &Error) -> poem::Error {
    let status = match err {
        Error::NotFound { .. }              => StatusCode::NOT_FOUND,
        Error::Invalid(_)                   => StatusCode::BAD_REQUEST,
        Error::Conflict(_)                  => StatusCode::CONFLICT,
        Error::Unauthorized                 => StatusCode::UNAUTHORIZED,
        Error::Upstream(_)                  => StatusCode::BAD_GATEWAY,
        Error::Storage(_) | Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let message = match err {
        Error::Upstream(_) => "an upstream service failed".to_owned(),
        Error::Storage(_) | Error::Other(_) => "internal error".to_owned(),
        client => client.to_string(),
    };
    let key = err.key(); // the variant's reverse-DNS error key
    request::record_error(key, status.is_server_error().then(|| err.to_string()));
    poem::Error::from_response(request::error_response(status, key, &message))
}
*/

// ──────────────── serializable, keyed errors (the trinity) ────────────────
// Errors that cross a serialization boundary (a lib* crate, a structured log, an
// API body) derive the trinity and carry a stable reverse-DNS ERROR KEY per variant:
//   dev.thmsn.<root>.<area…>.error.<kind>   ({{PRODUCT}} for the root; a lib drops `lib`)
// The key is the $type discriminant — specific to the code path, stable as a contract.
/*
use liberror::AnyError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, valuable::Valuable)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase",
        tag = "$type", content = "context")]
pub enum ConfigError {
    #[error("failed to read config at \"{path}\": {inner_error}")]
    #[serde(rename = "dev.thmsn.{{PRODUCT}}.server.config.error.read")]
    Read { path: String, inner_error: AnyError }, // AnyError wraps foreign errors, keeping the chain

    #[error("config was modified externally since it was last loaded")]
    #[serde(rename = "dev.thmsn.{{PRODUCT}}.server.config.error.stale")]
    Stale,
}
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;
*/
