//! Caching for content responses.
//!
//! The content is compiled in, so it is immutable for the life of the process and identical
//! for every process built from the same commit. Every successful content response
//! (`/api/v1/*`, `/manifest.json`, `/t/*`) therefore carries `ETag: "<commit>"` and
//! `Cache-Control: public, max-age=300`, and a request whose `If-None-Match` names that tag
//! gets `304 Not Modified`. One tag for every path is fine: a tag only has to change when
//! the resource at that URL changes, and nothing here changes without a new commit.
//!
//! The max-age is short on purpose: a deploy of a new commit should reach readers within
//! minutes, and revalidation against the tag is cheap.
//!
//! A build whose commit is `unknown` or whose tree was dirty sends neither header: its
//! commit does not pin the bytes, so a cache could keep serving an edit that is gone.

use poem::http::header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH};
use poem::http::{HeaderValue, StatusCode};
use poem::{Endpoint, IntoResponse, Middleware, Request, Response, Result};

pub const CACHE_CONTROL_VALUE: &str = "public, max-age=300";

/// The tag for content from `commit`, or `None` when `commit` does not identify the bytes.
#[must_use]
pub fn etag(commit: &str, dirty: bool) -> Option<String> {
    (!dirty && !commit.is_empty() && commit != standards_db::UNKNOWN_COMMIT)
        .then(|| format!("\"{commit}\""))
}

/// Whether an `If-None-Match` value names `etag` (a list, weak tags and `*` included).
fn matches(if_none_match: &str, etag: &str) -> bool {
    if_none_match.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

/// Adds the tag and cache headers to a 2xx and answers a matching revalidation with 304.
pub struct Immutable {
    etag: Option<String>,
}

impl Immutable {
    #[must_use]
    pub fn new(etag: Option<String>) -> Self {
        Self { etag }
    }
}

impl<E: Endpoint> Middleware<E> for Immutable {
    type Output = ImmutableEndpoint<E>;

    fn transform(&self, inner: E) -> Self::Output {
        ImmutableEndpoint {
            inner,
            etag: self
                .etag
                .as_deref()
                .and_then(|t| HeaderValue::from_str(t).ok()),
        }
    }
}

pub struct ImmutableEndpoint<E> {
    inner: E,
    etag: Option<HeaderValue>,
}

impl<E: Endpoint> Endpoint for ImmutableEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let Some(etag) = &self.etag else {
            return self.inner.call(req).await.map(IntoResponse::into_response);
        };
        let revalidating = req
            .headers()
            .get(IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            .zip(etag.to_str().ok())
            .is_some_and(|(inm, tag)| matches(inm, tag));
        let mut resp = self.inner.call(req).await?.into_response();
        if !resp.status().is_success() {
            return Ok(resp);
        }
        if revalidating {
            resp = StatusCode::NOT_MODIFIED.into_response();
        }
        resp.headers_mut().insert(ETAG, etag.clone());
        resp.headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_CONTROL_VALUE));
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_clean_known_commit_tags() {
        assert_eq!(etag("abc", false).as_deref(), Some("\"abc\""));
        assert_eq!(etag("abc", true), None);
        assert_eq!(etag("unknown", false), None);
        assert_eq!(etag("", false), None);
    }

    #[test]
    fn if_none_match_forms() {
        assert!(matches("\"abc\"", "\"abc\""));
        assert!(matches("W/\"abc\"", "\"abc\""));
        assert!(matches("\"x\", \"abc\"", "\"abc\""));
        assert!(matches("*", "\"abc\""));
        assert!(!matches("\"abd\"", "\"abc\""));
    }
}
