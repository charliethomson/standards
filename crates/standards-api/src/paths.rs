//! Path parameters that contain slashes.
//!
//! Doc ids (`archetypes/cli-tool`), template paths (`rust/build.rs`) and skill asset paths
//! (`scripts/obsq.sh`) have slashes, but a poem-openapi path parameter is one segment. The
//! contract names them as one parameter each (`/docs/{id}`, `/templates/{path}`,
//! `/skills/{name}/assets/{path}`), and a client sends the slash percent-encoded
//! (`/api/v1/docs/archetypes%2Fcli-tool`), which is what generated clients do with a path
//! parameter anyway. The router matches the raw path and decodes the parameter.
//!
//! So that a person with curl can type the slash too, [`encode_tail`] rewrites a raw-slash
//! request to the encoded form before routing: `/api/v1/docs/archetypes/cli-tool` and
//! `/api/v1/docs/archetypes%2Fcli-tool` reach the same handler. Only the tail after one of
//! the prefixes below is rewritten; nothing else is touched.

use poem::http::Uri;
use poem::{Endpoint, Request, Result};

/// `/api/v1/docs/{id}` and `/api/v1/templates/{path}`: everything after the prefix is the
/// one parameter.
const WHOLE_TAIL: [&str; 2] = ["/api/v1/docs/", "/api/v1/templates/"];
const SKILLS: &str = "/api/v1/skills/";
const ASSETS: &str = "/assets/";

/// The path with the multi-segment parameter's slashes encoded, or `None` when it needs no
/// rewrite.
#[must_use]
pub fn encode_tail(path: &str) -> Option<String> {
    let split_at = if let Some(prefix) = WHOLE_TAIL.iter().find(|p| path.starts_with(*p)) {
        prefix.len()
    } else {
        // /api/v1/skills/{name}/assets/{path..}: a skill name has no slash.
        let rest = path.strip_prefix(SKILLS)?;
        let (name, _) = rest.split_once('/')?;
        let after_name = SKILLS.len() + name.len();
        if !path[after_name..].starts_with(ASSETS) {
            return None;
        }
        after_name + ASSETS.len()
    };
    let (head, tail) = path.split_at(split_at);
    tail.contains('/')
        .then(|| format!("{head}{}", tail.replace('/', "%2F")))
}

/// Apply [`encode_tail`] to a request before it is routed. The query string is kept.
pub async fn rewrite<E: Endpoint>(ep: E, mut req: Request) -> Result<E::Output> {
    if let Some(path) = encode_tail(req.uri().path()) {
        let pq = match req.uri().query() {
            Some(q) => format!("{path}?{q}"),
            None => path,
        };
        let mut parts = req.uri().clone().into_parts();
        if let Ok(pq) = pq.parse() {
            parts.path_and_query = Some(pq);
            if let Ok(uri) = Uri::from_parts(parts) {
                *req.uri_mut() = uri;
            }
        }
    }
    ep.call(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_only_the_parameter_tail() {
        assert_eq!(
            encode_tail("/api/v1/docs/archetypes/cli-tool").as_deref(),
            Some("/api/v1/docs/archetypes%2Fcli-tool")
        );
        assert_eq!(encode_tail("/api/v1/docs/versioning"), None);
        assert_eq!(encode_tail("/api/v1/docs/archetypes%2Fcli-tool"), None);
        assert_eq!(
            encode_tail("/api/v1/templates/rust/build.rs").as_deref(),
            Some("/api/v1/templates/rust%2Fbuild.rs")
        );
        assert_eq!(
            encode_tail("/api/v1/skills/thmsn-logs/assets/scripts/obsq.sh").as_deref(),
            Some("/api/v1/skills/thmsn-logs/assets/scripts%2Fobsq.sh")
        );
        assert_eq!(encode_tail("/api/v1/skills/thmsn-logs/assets/x.sh"), None);
        assert_eq!(encode_tail("/api/v1/skills/thmsn-logs"), None);
        assert_eq!(encode_tail("/api/v1/skills/thmsn-logs/other/a/b"), None);
        assert_eq!(encode_tail("/api/v1/skills"), None);
        assert_eq!(encode_tail("/api/v1/search"), None);
        assert_eq!(encode_tail("/d/archetypes/cli-tool"), None);
    }
}
