// Service configuration via libconfig::Loader. Lives at api/src/config.rs.
// Precedence (later wins): defaults → TOML file → shared (unprefixed) env → prefixed env.
// Replace {{PRODUCT}} / {{PRODUCT_UPPER}}. See standards/docs/configuration.md.
//
// Cargo.toml:  libconfig = { git = "https://github.com/charliethomson/libconfig" }
//              libpath   = { git = "https://github.com/charliethomson/libpath" }   // for container paths

use libconfig::Loader;
use serde::{Deserialize, Serialize};

// Defaults are hardcoded HERE, not in a committed TOML.
const DEFAULT_BIND: &str = "127.0.0.1:8080"; // loopback locally; Docker sets {{PRODUCT_UPPER}}_BIND=0.0.0.0:8080
const DEFAULT_AUTH_TCP_ADDR: &str = "192.168.0.193:7070"; // homelab dev box

// Fleet-shared, unprefixed env vars every service reads.
const SHARED_ENV: [&str; 4] = ["AUTH_ADMIN_KEY", "AUTH_TCP_ADDR", "OTLP_ENDPOINT", "PRODUCTION"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: String,

    #[serde(default = "default_auth_tcp_addr")]
    pub auth_tcp_addr: String,

    // Required secret — provided via the shared env layer (Komodo writes it), never a file.
    pub auth_admin_key: String,

    #[serde(default)]
    pub otlp_endpoint: Option<String>,
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
}
fn default_auth_tcp_addr() -> String {
    DEFAULT_AUTH_TCP_ADDR.to_owned()
}

impl Config {
    /// Load from a deploy-controlled file (read-only, no mkdir) plus the service's own
    /// {{PRODUCT_UPPER}}_ vars and the bare fleet-shared vars. Use `Loader::pure_env()` for
    /// a no-file deploy. For desktop tools, use `Loader::module("{{PRODUCT}}")` instead.
    pub fn load(config_path: &str) -> Result<Self, libconfig::ConfigError> {
        Loader::path(config_path)
            .env_prefix("{{PRODUCT_UPPER}}_")
            .shared_env(SHARED_ENV)
            .load::<Self>()
    }
}

// Container path control (optional): set LIBPATH_BASE_DIR=/etc/{{PRODUCT}} and, if the config
// dir is read-only, `libpath::set_create_dirs(false)` — this also redirects liblog's log file.
