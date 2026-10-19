//! What every handler reaches through `Data<&AppState>`.

use std::sync::Arc;

use standards_core::{Bundle, Manifest};
use standards_engine::Engine;

use crate::caching;
use crate::config::Config;
use crate::metrics::Metrics;

#[derive(Clone)]
pub struct AppState {
    /// Search, checklists, docs, templates, skills over the embedded bundle.
    pub engine: Arc<Engine>,
    /// Computed once: the content cannot change while the process runs.
    pub manifest: Arc<Manifest>,
    /// The derived version of this build (`MAJOR.MINOR.<count>`).
    pub version: String,
    /// The `ETag` every content response carries; `None` when the content identity is not a
    /// clean commit ([`caching::etag`]).
    pub etag: Option<String>,
    pub metrics: Metrics,
    /// `Host` values the MCP endpoint accepts (`STANDARDS_ALLOWED_HOSTS`, L30).
    pub allowed_hosts: Arc<Vec<String>>,
}

impl AppState {
    /// State over `bundle`. `dirty` is whether this build came from a modified tree, which
    /// makes the commit an unreliable content identity.
    #[must_use]
    pub fn new(bundle: Arc<Bundle>, config: &Config, version: &str, dirty: bool) -> Self {
        let metrics = Metrics::new(version, &bundle);
        let etag = caching::etag(&bundle.commit, dirty);
        let engine = Engine::new(bundle);
        let manifest = Arc::new(engine.manifest(version));
        Self {
            engine: Arc::new(engine),
            manifest,
            version: version.to_owned(),
            etag,
            metrics,
            allowed_hosts: Arc::new(config.allowed_hosts()),
        }
    }
}
