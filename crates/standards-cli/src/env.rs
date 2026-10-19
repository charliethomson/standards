//! Everything the cli reads from the machine, gathered once in `main` and handed to
//! commands as an [`Env`], so tests inject a scratch home, cwd and exe instead of touching
//! the real ones.
//!
//! Configuration is env only, through `libconfig` (docs/configuration.md). Precedence for
//! the server host: `--host`, then `STANDARDS_HOST`, then [`DEFAULT_HOST`].
//!
//! | Env | Field | Default |
//! |---|---|---|
//! | `STANDARDS_HOST` | [`EnvConfig::host`] | `https://standards.thmsn.dev` |
//! | `STANDARDS_REGISTRY_URL` | [`EnvConfig::registry_url`] | `https://apps.dev.thmsn.dev` |
//! | `REGISTRY_TOKEN` (shared with the registry installer) | [`EnvConfig::registry_token`] | unset |

use std::ffi::OsString;
use std::path::PathBuf;

use libconfig::Loader;
use serde::{Deserialize, Serialize};

/// The public server (L4).
pub const DEFAULT_HOST: &str = standards_core::ORIGIN;

/// The registry the cli is published to (L10, docs/registry-publishing.md).
pub const DEFAULT_REGISTRY: &str = "https://apps.dev.thmsn.dev";

/// Fleet-shared, unprefixed vars: the registry installer reads the same token.
const SHARED_ENV: [&str; 1] = ["REGISTRY_TOKEN"];

/// The `STANDARDS_` vars and the shared ones. Every field is optional, so the derived
/// `Default` (libconfig's lowest layer) is all-unset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnvConfig {
    /// `STANDARDS_HOST`.
    #[serde(default)]
    pub host: Option<String>,
    /// `STANDARDS_REGISTRY_URL`.
    #[serde(default)]
    pub registry_url: Option<String>,
    /// `REGISTRY_TOKEN`: a bearer for the registry's reads.
    #[serde(default)]
    pub registry_token: Option<String>,
}

impl EnvConfig {
    /// Load from the process environment.
    pub fn load() -> Result<Self, libconfig::ConfigError> {
        Loader::pure_env()
            .env_prefix(standards_core::ENV_PREFIX)
            .shared_env(SHARED_ENV)
            .load::<Self>()
    }
}

/// The os/arch/libc triple the registry resolves an artifact for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    /// `darwin`, `linux`, `windows`.
    pub os: String,
    /// `amd64`, `arm64`.
    pub arch: String,
    /// `Some("musl")` on a musl build; the registry's default is `gnu`.
    pub libc: Option<String>,
}

impl Platform {
    /// The platform this binary was built for, in the registry's names.
    #[must_use]
    pub fn current() -> Self {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            other => other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };
        Self {
            os: os.to_owned(),
            arch: arch.to_owned(),
            libc: cfg!(target_env = "musl").then(|| "musl".to_owned()),
        }
    }
}

/// The machine, as commands see it.
#[derive(Debug, Clone)]
pub struct Env {
    /// `$HOME`.
    pub home: PathBuf,
    /// `$XDG_CACHE_HOME`, when set to an absolute path.
    pub xdg_cache_home: Option<PathBuf>,
    /// Where relative paths resolve and where `init` looks for the repo.
    pub cwd: PathBuf,
    /// The running binary, which `self-update` replaces.
    pub current_exe: Option<PathBuf>,
    /// The running cli's derived version.
    pub version: String,
    pub platform: Platform,
    pub config: EnvConfig,
    /// `$PATH`, where `doctor` looks for `claude` (and what it hands the child).
    pub path: Option<OsString>,
}

impl Env {
    /// The real process environment. Fails only when the env config does not parse.
    pub fn from_process() -> Result<Self, libconfig::ConfigError> {
        let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
        Ok(Self {
            home: var("HOME").unwrap_or_default(),
            xdg_cache_home: var("XDG_CACHE_HOME"),
            cwd: std::env::current_dir().unwrap_or_default(),
            current_exe: std::env::current_exe().ok(),
            version: standards_core::VERSION.to_owned(),
            platform: Platform::current(),
            config: EnvConfig::load()?,
            path: std::env::var_os("PATH"),
        })
    }

    /// `--host`, else `STANDARDS_HOST`, else [`DEFAULT_HOST`].
    #[must_use]
    pub fn host(&self, flag: Option<&str>) -> String {
        flag.or(self.config.host.as_deref())
            .filter(|h| !h.trim().is_empty())
            .unwrap_or(DEFAULT_HOST)
            .to_owned()
    }

    /// `STANDARDS_REGISTRY_URL`, else [`DEFAULT_REGISTRY`].
    #[must_use]
    pub fn registry_url(&self) -> &str {
        self.config
            .registry_url
            .as_deref()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or(DEFAULT_REGISTRY)
    }

    /// `${XDG_CACHE_HOME:-~/.cache}/thmsn/standards`. A relative `XDG_CACHE_HOME` is
    /// ignored, as the XDG spec says.
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.xdg_cache_home
            .as_ref()
            .filter(|p| p.is_absolute())
            .cloned()
            .unwrap_or_else(|| self.home.join(".cache"))
            .join("thmsn")
            .join("standards")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            home: PathBuf::from("/h"),
            xdg_cache_home: None,
            cwd: PathBuf::from("/w"),
            current_exe: None,
            version: "0.1.0".into(),
            platform: Platform::current(),
            config: EnvConfig::default(),
            path: None,
        }
    }

    #[test]
    fn host_precedence() {
        let mut e = env();
        assert_eq!(e.host(None), "https://standards.thmsn.dev");
        e.config.host = Some("http://env".into());
        assert_eq!(e.host(None), "http://env");
        assert_eq!(e.host(Some("http://flag")), "http://flag");
        e.config.host = Some("  ".into());
        assert_eq!(e.host(None), DEFAULT_HOST);
        assert_eq!(e.registry_url(), DEFAULT_REGISTRY);
        e.config.registry_url = Some("http://reg".into());
        assert_eq!(e.registry_url(), "http://reg");
    }

    #[test]
    fn cache_dir_honours_absolute_xdg_only() {
        let mut e = env();
        assert_eq!(e.cache_dir(), PathBuf::from("/h/.cache/thmsn/standards"));
        e.xdg_cache_home = Some(PathBuf::from("rel"));
        assert_eq!(e.cache_dir(), PathBuf::from("/h/.cache/thmsn/standards"));
        e.xdg_cache_home = Some(PathBuf::from("/x"));
        assert_eq!(e.cache_dir(), PathBuf::from("/x/thmsn/standards"));
    }

    #[test]
    fn platform_uses_registry_names() {
        let p = Platform::current();
        assert!(["darwin", "linux", "windows"].contains(&p.os.as_str()) || !p.os.is_empty());
        assert!(!["x86_64", "aarch64", "macos"].contains(&p.arch.as_str()));
        assert_ne!(p.os, "macos");
    }

    #[test]
    fn loads_from_the_process_env() {
        // Reads whatever the test process has; it must parse, and nothing here sets vars.
        let e = Env::from_process().unwrap();
        assert_eq!(e.version, standards_core::VERSION);
    }
}
