// Service configuration via libconfig::Loader. Lives at api/src/config.rs.
// Precedence (later wins): defaults → TOML file → shared (unprefixed) env → prefixed env.
// Replace {{PRODUCT}} / {{PRODUCT_UPPER}}. See https://standards.thmsn.dev/d/configuration.
//
// Cargo.toml:  libconfig = { git = "https://github.com/charliethomson/libconfig" }
//              libpath   = { git = "https://github.com/charliethomson/libpath" }   // for container paths

use libconfig::Loader;
use serde::{Deserialize, Serialize};

// Defaults are hardcoded HERE, not in a committed TOML.
const DEFAULT_BIND: &str = "127.0.0.1:8080"; // loopback locally; Docker sets {{PRODUCT_UPPER}}_BIND=0.0.0.0:8080
// OIDC (https://standards.thmsn.dev/d/auth-integration). The issuer is compared exactly with
// the token's `iss`; the audience is this product's app id.
const DEFAULT_OIDC_ISSUER: &str = "https://sso.dev.thmsn.dev";
const DEFAULT_OIDC_AUDIENCE: &str = "dev.thmsn.{{PRODUCT}}";

// Fleet-shared, UNPREFIXED env vars every service reads. Auth config deliberately does NOT
// belong here: {{PRODUCT_UPPER}}_AUTH_MODE and {{PRODUCT_UPPER}}_OIDC_* are this service's own
// fields. Listing bare names here fails silently: they're ignored and the defaults apply.
const SHARED_ENV: [&str; 2] = ["OTLP_ENDPOINT", "PRODUCTION"];

/// Declared, never inferred. `Oidc` refuses to boot on a blank issuer/audience or an
/// unreachable JWKS; `None` is loopback-only and loud; `Proxied` trusts one upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    #[default]
    Oidc,
    None,
    Proxied,
}

// NOTE: do NOT `#[derive(Default)]` here. libconfig's lowest precedence layer is
// `Serialized::defaults(Config::default())` — it serializes this `Default` impl, so a derived
// one supplies `""`/`0`/`None` for every field and the `#[serde(default = "...")]` functions
// below never run. Every DEFAULT_* const would be dead and `bind` would silently become "".
// Write `Default` by hand and delegate to the same functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: String,

    #[serde(default)]
    pub auth_mode: AuthMode,

    #[serde(default = "default_oidc_issuer")]
    pub oidc_issuer: String,

    #[serde(default = "default_oidc_audience")]
    pub oidc_audience: String,

    // Client ids allowed on service routes ({{PRODUCT_UPPER}}_OIDC_SERVICE_CLIENTS). Empty = none.
    #[serde(default)]
    pub oidc_service_clients: Vec<String>,

    // Web server-side exchange only: the `{{PRODUCT}}-web` client's secret. Komodo writes it;
    // never a file. Required when the product serves a web client.
    #[serde(default)]
    pub oidc_client_secret: Option<String>,

    #[serde(default)]
    pub otlp_endpoint: Option<String>,
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
}
fn default_oidc_issuer() -> String {
    DEFAULT_OIDC_ISSUER.to_owned()
}
fn default_oidc_audience() -> String {
    DEFAULT_OIDC_AUDIENCE.to_owned()
}

// The real default layer. Keep every field in sync with the `#[serde(default = "...")]`
// attributes above — this impl, not those attributes, is what libconfig actually reads.
impl Default for Config {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            auth_mode: AuthMode::default(),
            oidc_issuer: default_oidc_issuer(),
            oidc_audience: default_oidc_audience(),
            oidc_service_clients: Vec::new(),
            oidc_client_secret: None,
            otlp_endpoint: None,
        }
    }
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
