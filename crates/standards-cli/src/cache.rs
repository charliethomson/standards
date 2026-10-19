//! The manifest cache at `${XDG_CACHE_HOME:-~/.cache}/thmsn/standards/manifest.json`.
//!
//! `doctor --quiet` runs on every Claude session start, so it must not wait on the network:
//! it reads this file, refreshes it at most every [`MAX_AGE`] with a [`QUIET_TIMEOUT`]
//! budget, and falls back to the stale copy when the refresh fails. The file records the
//! host it came from; a copy from another host is never served.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use standards_core::Manifest;

use crate::client::{Client, ClientError};
use crate::env::Env;

/// How long a cached manifest is trusted before a refresh is attempted.
pub const MAX_AGE: Duration = Duration::from_hours(1);

/// The refresh budget on the `--quiet` path.
pub const QUIET_TIMEOUT: Duration = Duration::from_secs(1);

/// The file's contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedManifest {
    /// The server it was fetched from, without a trailing slash.
    pub host: String,
    /// Unix seconds.
    pub fetched_at: u64,
    pub manifest: Manifest,
}

/// Where a [`ManifestCache::load`] answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// The cached copy was young enough; no request was made.
    Cached,
    /// Fetched just now and written to the cache.
    Fetched,
    /// The refresh failed; this is the last copy from the same host.
    Stale,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub manifest: Manifest,
    pub freshness: Freshness,
    /// When the manifest was fetched (Unix seconds).
    pub fetched_at: u64,
}

#[derive(Debug, Clone)]
pub struct ManifestCache {
    path: PathBuf,
}

impl ManifestCache {
    /// The cache for this machine.
    #[must_use]
    pub fn for_env(env: &Env) -> Self {
        Self::at(env.cache_dir().join("manifest.json"))
    }

    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file, if it exists and parses.
    #[must_use]
    pub fn read(&self) -> Option<CachedManifest> {
        let bytes = std::fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// The cached manifest for `host` if it is younger than `max_age` at `now`.
    #[must_use]
    pub fn fresh(&self, host: &str, now: SystemTime, max_age: Duration) -> Option<CachedManifest> {
        self.read()
            .filter(|c| c.host == trim(host) && age(c.fetched_at, now) <= max_age)
    }

    /// Replace the file atomically (a temp file in the same directory, then a rename).
    pub fn write(&self, host: &str, manifest: &Manifest, now: SystemTime) -> std::io::Result<()> {
        let cached = CachedManifest {
            host: trim(host).to_owned(),
            fetched_at: unix(now),
            manifest: manifest.clone(),
        };
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".manifest.json.{}", std::process::id()));
        let json = serde_json::to_vec_pretty(&cached).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// The manifest for `client`'s host: the cached copy while it is younger than
    /// `max_age`, else a fresh fetch (written back), else the stale copy. Errors only when
    /// the fetch fails and there is no copy from this host at all. A failed cache write is
    /// ignored: the answer is still good.
    pub fn load(
        &self,
        client: &Client,
        now: SystemTime,
        max_age: Duration,
    ) -> Result<Loaded, ClientError> {
        if let Some(c) = self.fresh(client.host(), now, max_age) {
            return Ok(Loaded {
                manifest: c.manifest,
                freshness: Freshness::Cached,
                fetched_at: c.fetched_at,
            });
        }
        match client.manifest() {
            Ok(manifest) => {
                let _ = self.write(client.host(), &manifest, now);
                Ok(Loaded {
                    manifest,
                    freshness: Freshness::Fetched,
                    fetched_at: unix(now),
                })
            }
            Err(error) => match self.read().filter(|c| c.host == client.host()) {
                Some(c) => Ok(Loaded {
                    manifest: c.manifest,
                    freshness: Freshness::Stale,
                    fetched_at: c.fetched_at,
                }),
                None => Err(error),
            },
        }
    }
}

fn trim(host: &str) -> &str {
    host.trim_end_matches('/')
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// How old a fetch at `fetched_at` is at `now`; a timestamp in the future counts as new.
fn age(fetched_at: u64, now: SystemTime) -> Duration {
    Duration::from_secs(unix(now).saturating_sub(fetched_at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(commit: &str) -> Manifest {
        Manifest {
            schema: 1,
            commit: commit.into(),
            server_version: "0.1.1".into(),
            min_cli_version: "0.0.0".into(),
            cli_install_url: standards_core::CLI_INSTALL_URL.into(),
            archetypes: vec!["library".into()],
            skills: vec![],
        }
    }

    #[test]
    fn round_trips_and_ages_out() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ManifestCache::at(dir.path().join("a/b/manifest.json"));
        assert!(cache.read().is_none());
        let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
        cache.write("http://h/", &manifest("c1"), t0).unwrap();
        let read = cache.read().unwrap();
        assert_eq!(read.host, "http://h");
        assert_eq!(read.fetched_at, 1_000_000);

        let fresh = |host, secs| cache.fresh(host, t0 + Duration::from_secs(secs), MAX_AGE);
        assert!(fresh("http://h", 0).is_some());
        assert!(fresh("http://h/", 3600).is_some());
        assert!(fresh("http://h", 3601).is_none());
        assert!(fresh("http://other", 0).is_none());
        // A clock that went backwards still trusts the copy.
        assert!(cache.fresh("http://h", UNIX_EPOCH, MAX_AGE).is_some());
    }

    #[test]
    fn garbage_is_no_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(ManifestCache::at(path).read().is_none());
    }

    #[test]
    fn stale_copy_when_the_server_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ManifestCache::at(dir.path().join("manifest.json"));
        let host = crate::testing::dead_host();
        let client = Client::new(&host, QUIET_TIMEOUT).unwrap();
        let now = SystemTime::now();

        let err = cache.load(&client, now, MAX_AGE).unwrap_err();
        assert!(err.is_unreachable());

        cache
            .write(&host, &manifest("old"), now - Duration::from_secs(7200))
            .unwrap();
        let loaded = cache.load(&client, now, MAX_AGE).unwrap();
        assert_eq!(loaded.freshness, Freshness::Stale);
        assert_eq!(loaded.manifest.commit, "old");

        let loaded = cache.load(&client, now, Duration::from_hours(3)).unwrap();
        assert_eq!(loaded.freshness, Freshness::Cached);
    }

    #[test]
    fn unwritable_cache_dir_is_an_error_on_write() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        std::fs::write(&file, "x").unwrap();
        let cache = ManifestCache::at(file.join("manifest.json"));
        assert!(
            cache
                .write("http://h", &manifest("c"), SystemTime::now())
                .is_err()
        );
    }
}
