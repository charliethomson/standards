//! Service configuration via `libconfig::Loader` (docs/configuration.md).
//!
//! No file: the server is configured by env alone (`Loader::pure_env()`). Precedence
//! (later wins): defaults, then the shared unprefixed vars, then `STANDARDS_` vars.
//!
//! | Env | Field | Default |
//! |---|---|---|
//! | `STANDARDS_BIND` | [`Config::bind`] | `127.0.0.1:8080` |
//! | `STANDARDS_ALLOWED_HOSTS` | [`Config::allowed_hosts`] | `standards.thmsn.dev,localhost,127.0.0.1,::1` |
//! | `OTLP_ENDPOINT` | [`Config::otlp_endpoint`] | unset (no export) |
//! | `PRODUCTION` | [`Config::production`] | `false` |
//! | `SAMPLE_RATE` | [`Config::sample_rate`] | unset (always on) |

use libconfig::Loader;
use serde::{Deserialize, Serialize};

// Defaults are hardcoded HERE, not in a committed TOML.
/// Loopback locally; the image sets `STANDARDS_BIND=0.0.0.0:8080`.
const DEFAULT_BIND: &str = "127.0.0.1:8080";
/// The public hostname plus loopback, so `/mcp` answers behind Caddy and locally.
const DEFAULT_ALLOWED_HOSTS: &str = "standards.thmsn.dev,localhost,127.0.0.1,::1";

/// Fleet-shared, unprefixed vars. No auth vars: the server is public and read-only (L4).
const SHARED_ENV: [&str; 3] = ["OTLP_ENDPOINT", "PRODUCTION", "SAMPLE_RATE"];

// Not derived: libconfig serializes `Default` as its lowest layer, so a derived one
// would make every `#[serde(default = "...")]` below dead (docs/configuration.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// `STANDARDS_BIND`: the listen address.
    #[serde(default = "default_bind")]
    pub bind: String,

    /// `STANDARDS_ALLOWED_HOSTS`: the `Host` values the MCP endpoint accepts (DNS-rebinding
    /// protection, L30), comma or space separated. Read through [`Config::allowed_hosts`].
    #[serde(default = "default_allowed_hosts")]
    pub allowed_hosts: String,

    /// `OTLP_ENDPOINT` (shared): the collector; unset means no export.
    #[serde(default)]
    pub otlp_endpoint: Option<String>,

    /// `PRODUCTION` (shared): the deployment environment.
    #[serde(default)]
    pub production: bool,

    /// `SAMPLE_RATE` (shared): a head-sampling override for local work only.
    #[serde(default)]
    pub sample_rate: Option<f64>,
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
}

fn default_allowed_hosts() -> String {
    DEFAULT_ALLOWED_HOSTS.to_owned()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            allowed_hosts: default_allowed_hosts(),
            otlp_endpoint: None,
            production: false,
            sample_rate: None,
        }
    }
}

impl Config {
    /// Load from the environment: the `STANDARDS_` vars and the shared fleet vars.
    pub fn load() -> Result<Self, libconfig::ConfigError> {
        Loader::pure_env()
            .env_prefix(standards_core::ENV_PREFIX)
            .shared_env(SHARED_ENV)
            .load::<Self>()
    }

    /// The allowed hosts as a list: split on commas and whitespace, blanks dropped.
    #[must_use]
    pub fn allowed_hosts(&self) -> Vec<String> {
        self.allowed_hosts
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|h| !h.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Always on unless `SAMPLE_RATE` pins a ratio: the collector tail-samples
    /// (docs/tracing.md).
    #[must_use]
    pub fn sampling(&self) -> liblog::Sampling {
        match self.sample_rate {
            Some(rate) => liblog::Sampling::Ratio(rate),
            None => liblog::Sampling::AlwaysOn,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;

    const ALL_VARS: [&str; 5] = [
        "STANDARDS_BIND",
        "STANDARDS_ALLOWED_HOSTS",
        "OTLP_ENDPOINT",
        "PRODUCTION",
        "SAMPLE_RATE",
    ];

    /// The process environment is global and tests run threaded: serialize.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn load_with(vars: &[(&str, &str)]) -> Config {
        let _guard = env_lock();
        let saved: Vec<_> = ALL_VARS
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
        // SAFETY: `env_lock` gives this thread exclusive use of the environment.
        unsafe {
            for key in ALL_VARS {
                std::env::remove_var(key);
            }
            for (key, value) in vars {
                std::env::set_var(key, value);
            }
        }
        let config = Config::load().expect("env-only load");
        // SAFETY: as above, still under the lock.
        unsafe {
            for (key, value) in saved {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
        config
    }

    #[test]
    fn defaults_apply_with_no_env() {
        let config = load_with(&[]);
        assert_eq!(config, Config::default());
        assert_eq!(config.bind, "127.0.0.1:8080");
        assert_eq!(
            config.allowed_hosts(),
            ["standards.thmsn.dev", "localhost", "127.0.0.1", "::1"]
        );
        assert!(matches!(config.sampling(), liblog::Sampling::AlwaysOn));
    }

    #[test]
    fn prefixed_and_shared_vars_apply() {
        let config = load_with(&[
            ("STANDARDS_BIND", "0.0.0.0:9000"),
            ("STANDARDS_ALLOWED_HOSTS", "a.example, b.example  localhost"),
            ("OTLP_ENDPOINT", "http://collector:4317"),
            ("PRODUCTION", "true"),
            ("SAMPLE_RATE", "0.5"),
        ]);
        assert_eq!(config.bind, "0.0.0.0:9000");
        assert_eq!(
            config.allowed_hosts(),
            ["a.example", "b.example", "localhost"]
        );
        assert_eq!(
            config.otlp_endpoint.as_deref(),
            Some("http://collector:4317")
        );
        assert!(config.production);
        assert!(
            matches!(config.sampling(), liblog::Sampling::Ratio(r) if (r - 0.5).abs() < f64::EPSILON)
        );
    }
}
