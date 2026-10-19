//! The only place engine errors meet HTTP (docs/error-handling.md).
//!
//! Every engine error is a lookup that missed, so every one is a 404. The body is the JSON
//! every client reads, `{"message", "$type", "traceId"}`: `message` is the error's own
//! `Display` (written for the caller, e.g. "no doc `x`"), `$type` a stable key naming the miss.

use poem::http::StatusCode;
use standards_engine::Error;

use crate::dto::ErrorBody;
use crate::request;

/// The stable error key for `err`: `dev.thmsn.standards.server.content.error.<kind>`.
#[must_use]
pub fn key(err: &Error) -> &'static str {
    match err {
        Error::UnknownDoc { .. } => "dev.thmsn.standards.server.content.error.unknown_doc",
        Error::UnknownSection { .. } => "dev.thmsn.standards.server.content.error.unknown_section",
        Error::UnknownArchetype { .. } => {
            "dev.thmsn.standards.server.content.error.unknown_archetype"
        }
        Error::UnknownSkill { .. } => "dev.thmsn.standards.server.content.error.unknown_skill",
        Error::UnknownAsset { .. } => "dev.thmsn.standards.server.content.error.unknown_asset",
        Error::UnknownTemplate { .. } => {
            "dev.thmsn.standards.server.content.error.unknown_template"
        }
    }
}

/// `err` as the body of a documented 404, recorded on the request span. No log here: the
/// request layer's `http completed` line carries it.
#[must_use]
pub fn body(err: &Error) -> ErrorBody {
    let kind = key(err);
    request::record_error(kind, None);
    ErrorBody {
        message: err.to_string(),
        kind: kind.to_owned(),
        trace_id: request::current_trace_id(),
    }
}

/// `err` as a 404 for the routes outside the contract (`/t/{path}`).
#[must_use]
pub fn to_poem(err: &Error) -> poem::Error {
    let key = key(err);
    request::record_error(key, None);
    poem::Error::from_response(request::error_response(
        StatusCode::NOT_FOUND,
        key,
        &err.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_variant_is_a_keyed_404() {
        let s = String::from;
        let cases = [
            Error::UnknownDoc { id: s("x") },
            Error::UnknownSection {
                doc: s("x"),
                anchor: s("y"),
            },
            Error::UnknownArchetype { name: s("x") },
            Error::UnknownSkill { name: s("x") },
            Error::UnknownAsset {
                skill: s("x"),
                path: s("y"),
            },
            Error::UnknownTemplate { path: s("x") },
        ];
        let mut keys = std::collections::BTreeSet::new();
        for err in cases {
            let resp = to_poem(&err).into_response();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            let json: serde_json::Value = resp.into_body().into_json().await.unwrap();
            assert_eq!(json["$type"], key(&err));
            assert_eq!(json["message"], err.to_string());
            assert!(key(&err).starts_with("dev.thmsn.standards.server.content.error."));
            let b = body(&err);
            assert_eq!((b.kind.as_str(), b.message), (key(&err), err.to_string()));
            keys.insert(key(&err));
        }
        assert_eq!(keys.len(), 6, "keys are distinct");
    }
}
