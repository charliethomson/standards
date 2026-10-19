//! A blocking client for the server's REST mirror (L8: the cli uses REST, not MCP).
//!
//! One request at a time with a total timeout per client. Failures are typed so a caller
//! can tell "the server is not there" ([`ClientError::is_unreachable`], exit 3) from "the
//! server said no" (a 404 naming what was missing, exit 1).

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Response;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use standards_core::Manifest;

/// The budget for an ordinary command's requests.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// What the server sends with a 404 (`{message, $type, traceId}`).
#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: String,
}

/// `GET /api/v1/templates` entries.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TemplateSummary {
    pub path: String,
    pub placeholders: Vec<String>,
}

/// `GET /api/v1/templates/{path}`: the body with the given keys filled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RenderedTemplate {
    pub path: String,
    pub body: String,
    /// Placeholders left in `body` because no value was given.
    pub unfilled: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("`{0}` is not an http(s) URL")]
    BadHost(String),
    /// Connection refused, DNS, TLS, a timeout, or a gateway saying the server is down.
    #[error("can't reach {host}: {reason}")]
    Unreachable { host: String, reason: String },
    /// A 404 with the server's message.
    #[error("{message}")]
    NotFound { url: String, message: String },
    #[error("{url}: HTTP {status}")]
    Status { url: String, status: u16 },
    #[error("{url}: unexpected response: {reason}")]
    Decode { url: String, reason: String },
}

impl ClientError {
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable { .. })
    }

    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::BadHost(_) => "dev.thmsn.standards.cli.args.error.bad_host",
            Self::Unreachable { .. } => "dev.thmsn.standards.cli.server.error.unreachable",
            Self::NotFound { .. } => "dev.thmsn.standards.cli.server.error.not_found",
            Self::Status { .. } => "dev.thmsn.standards.cli.server.error.status",
            Self::Decode { .. } => "dev.thmsn.standards.cli.server.error.bad_response",
        }
    }
}

/// The standards server at one host.
#[derive(Debug, Clone)]
pub struct Client {
    base: Url,
    http: reqwest::blocking::Client,
}

impl Client {
    /// A client for `host` (`https://standards.thmsn.dev`, a trailing slash is fine) whose
    /// requests each give up after `timeout`.
    pub fn new(host: &str, timeout: Duration) -> Result<Self, ClientError> {
        let base = parse_base(host).ok_or_else(|| ClientError::BadHost(host.to_owned()))?;
        let http = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout)
            .user_agent(concat!("standards-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ClientError::BadHost(format!("{host} ({e})")))?;
        Ok(Self { base, http })
    }

    /// The host as given, without a trailing slash.
    #[must_use]
    pub fn host(&self) -> &str {
        self.base.as_str().trim_end_matches('/')
    }

    /// `GET /manifest.json`.
    pub fn manifest(&self) -> Result<Manifest, ClientError> {
        self.get_json(&self.url(&["manifest.json"], &[]))
    }

    /// `GET /api/v1/templates`.
    pub fn templates(&self) -> Result<Vec<TemplateSummary>, ClientError> {
        self.get_json(&self.url(&["api", "v1", "templates"], &[]))
    }

    /// `GET /api/v1/templates/{path}?KEY=V..`: the server fills the given keys (L8).
    pub fn render_template(
        &self,
        path: &str,
        vars: &BTreeMap<String, String>,
    ) -> Result<RenderedTemplate, ClientError> {
        let query: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        self.get_json(&self.url(&["api", "v1", "templates", path], &query))
    }

    /// `GET` any path under the host as JSON. Each segment is one path segment, so a
    /// slash inside one (`archetypes/cli-tool`) is sent percent-encoded.
    pub fn get<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<T, ClientError> {
        self.get_json(&self.url(segments, query))
    }

    fn url(&self, segments: &[&str], query: &[(&str, &str)]) -> Url {
        let mut url = self.base.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        url
    }

    fn get_json<T: DeserializeOwned>(&self, url: &Url) -> Result<T, ClientError> {
        let response = self
            .http
            .get(url.clone())
            .send()
            .map_err(|e| self.unreachable(&e))?;
        let response = self.check(response)?;
        response.json().map_err(|e| ClientError::Decode {
            url: url.to_string(),
            reason: e.to_string(),
        })
    }

    fn check(&self, response: Response) -> Result<Response, ClientError> {
        let status = response.status();
        let url = response.url().to_string();
        if status.is_success() {
            return Ok(response);
        }
        if matches!(
            status,
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
        ) {
            return Err(ClientError::Unreachable {
                host: self.host().to_owned(),
                reason: format!("HTTP {}", status.as_u16()),
            });
        }
        if status == StatusCode::NOT_FOUND {
            let message = response
                .json::<ErrorBody>()
                .map_or_else(|_| format!("{url}: not found"), |b| b.message);
            return Err(ClientError::NotFound { url, message });
        }
        Err(ClientError::Status {
            url,
            status: status.as_u16(),
        })
    }

    fn unreachable(&self, error: &reqwest::Error) -> ClientError {
        ClientError::Unreachable {
            host: self.host().to_owned(),
            reason: describe(error),
        }
    }
}

/// `http(s)://host[:port][/prefix]`, normalised to end in `/`.
fn parse_base(host: &str) -> Option<Url> {
    let mut url = Url::parse(host.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return None;
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

/// A transport error in a few words: reqwest's own message is a chain of wrappers.
#[must_use]
pub fn describe(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return "timed out".to_owned();
    }
    if error.is_connect() {
        return "connection failed".to_owned();
    }
    let mut source: Option<&dyn std::error::Error> = Some(error);
    let mut last = error.to_string();
    while let Some(e) = source {
        last = e.to_string();
        source = e.source();
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_normalise_and_reject_non_http() {
        let c = Client::new("http://127.0.0.1:9/", DEFAULT_TIMEOUT).unwrap();
        assert_eq!(c.host(), "http://127.0.0.1:9");
        let c = Client::new("https://example.test/prefix?x=1", DEFAULT_TIMEOUT).unwrap();
        assert_eq!(c.host(), "https://example.test/prefix");
        assert_eq!(
            c.url(
                &["api", "v1", "templates", "rust/build.rs"],
                &[("PRODUCT", "a b")]
            )
            .as_str(),
            "https://example.test/prefix/api/v1/templates/rust%2Fbuild.rs?PRODUCT=a+b"
        );
        for bad in ["ftp://x", "not a url", "file:///tmp", ""] {
            let e = Client::new(bad, DEFAULT_TIMEOUT).unwrap_err();
            assert!(matches!(e, ClientError::BadHost(_)), "{bad}");
            assert!(!e.is_unreachable());
        }
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        // Port 9 (discard) is not served on loopback in any test environment we run.
        let c = Client::new("http://127.0.0.1:9", Duration::from_secs(2)).unwrap();
        let e = c.manifest().unwrap_err();
        assert!(e.is_unreachable(), "{e}");
        assert_eq!(e.key(), "dev.thmsn.standards.cli.server.error.unreachable");
    }
}
