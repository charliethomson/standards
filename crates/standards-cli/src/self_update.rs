//! `standards self-update [--check]`: replace this binary with the registry's latest.
//!
//! The registry (L10, docs/registry-publishing.md) publishes the cli as product
//! `dev.thmsn.standards`, app `cli` (kind `cli`). The flow:
//!
//! 1. `GET {registry}/api/v1/products/dev.thmsn.standards/apps/cli/resolve?os=&arch=[&libc=]`
//!    picks the newest published, unyanked `stable` version for this platform and returns
//!    `{version, target, download_url, file_name, sha256, file_size, bin_name}`. Reads
//!    need a bearer: `REGISTRY_TOKEN`, the same variable the registry's installer reads.
//! 2. Compare with the running derived version; `--check` (or already current) stops here.
//! 3. Download `download_url` (pre-signed, no auth), check its size and sha256, take the
//!    binary out of the `.tar.gz` (the entry named `bin_name`, default `standards`), or use
//!    the file as is when it is not an archive.
//! 4. Write it beside the running exe as a temp file, make it executable, and rename it over
//!    the exe: the swap is atomic, and a failure leaves the old binary in place.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use standards_core::DerivedVersion;

use crate::Ctx;
use crate::client::describe;
use crate::env::Platform;
use crate::error::{Error, Result};
use crate::output::Outcome;

/// The registry product and app the release pipeline publishes to (L3).
pub const PRODUCT: &str = "dev.thmsn.standards";
pub const APP: &str = "cli";
/// The binary's name inside an artifact when the registry does not say.
pub const BIN_NAME: &str = "standards";

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// Only report whether an update is available
    #[arg(long)]
    pub check: bool,
}

/// The registry's `resolve` answer (the fields the cli uses).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Resolved {
    pub version: String,
    pub target: String,
    pub download_url: String,
    pub file_name: String,
    pub sha256: String,
    #[serde(default)]
    pub file_size: Option<u64>,
    #[serde(default)]
    pub bin_name: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("`{0}` is not an http(s) URL")]
    BadUrl(String),
    #[error("can't reach the registry at {url}: {reason}")]
    Unreachable { url: String, reason: String },
    #[error(
        "the registry refused this machine (HTTP {status}); set REGISTRY_TOKEN, or run the installer once to authorize it"
    )]
    Unauthorized { status: u16 },
    #[error("no published {PRODUCT}/{APP} for {target}")]
    NotPublished { target: String },
    #[error("registry: HTTP {status} from {url}")]
    Status { url: String, status: u16 },
    #[error("registry: unexpected response: {0}")]
    BadResponse(String),
    #[error("download does not match the registry: {0}")]
    Mismatch(String),
}

impl RegistryError {
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable { .. })
    }

    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::BadUrl(_) => "dev.thmsn.standards.cli.registry.error.bad_url",
            Self::Unreachable { .. } => "dev.thmsn.standards.cli.registry.error.unreachable",
            Self::Unauthorized { .. } => "dev.thmsn.standards.cli.registry.error.unauthorized",
            Self::NotPublished { .. } => "dev.thmsn.standards.cli.registry.error.not_published",
            Self::Status { .. } => "dev.thmsn.standards.cli.registry.error.status",
            Self::BadResponse(_) => "dev.thmsn.standards.cli.registry.error.bad_response",
            Self::Mismatch(_) => "dev.thmsn.standards.cli.registry.error.mismatch",
        }
    }
}

pub fn run(ctx: &Ctx, args: &Args) -> Result<Outcome> {
    let registry = ctx.env.registry_url();
    let token = ctx.env.config.registry_token.as_deref();
    let resolved = resolve(registry, &ctx.env.platform, token)?;

    let current = &ctx.env.version;
    let available = is_newer(&resolved.version, current)?;
    let mut data = json!({
        "current": current,
        "latest": resolved.version,
        "target": resolved.target,
        "update_available": available,
        "installed": false,
    });

    if args.check || !available {
        let line = if available {
            format!(
                "standards {} is available (running {current}); run `standards self-update`",
                resolved.version
            )
        } else {
            format!("standards {current} is current")
        };
        return Ok(Outcome::ok(data).line(line));
    }

    let exe = ctx
        .env
        .current_exe
        .clone()
        .ok_or_else(|| Error::Fail("can't locate the running binary".to_owned()))?;
    let bytes = download(registry, &resolved)?;
    let binary = extract(&resolved, bytes)?;
    replace(&exe, &binary)?;

    data["installed"] = json!(true);
    data["path"] = json!(exe.display().to_string());
    Ok(Outcome::ok(data).line(format!(
        "updated standards {current} -> {} ({})",
        resolved.version,
        exe.display()
    )))
}

/// Whether `latest` is newer than `current`. An unparsable `current` (a hand-built binary)
/// is treated as older than anything.
fn is_newer(latest: &str, current: &str) -> Result<bool> {
    let latest: DerivedVersion = latest
        .parse()
        .map_err(|e: String| RegistryError::BadResponse(e))?;
    Ok(current
        .parse::<DerivedVersion>()
        .map_or(true, |current| latest > current))
}

fn http(timeout: Duration) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout.min(Duration::from_secs(10)))
        .user_agent(concat!("standards-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| RegistryError::BadResponse(e.to_string()).into())
}

fn base(registry: &str) -> Result<Url> {
    let url = Url::parse(registry.trim_end_matches('/'))
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some())
        .ok_or_else(|| RegistryError::BadUrl(registry.to_owned()))?;
    Ok(url)
}

/// The newest published version for `platform`.
pub fn resolve(registry: &str, platform: &Platform, token: Option<&str>) -> Result<Resolved> {
    let mut url = base(registry)?;
    if let Ok(mut path) = url.path_segments_mut() {
        path.pop_if_empty()
            .extend(["api", "v1", "products", PRODUCT, "apps", APP, "resolve"]);
    }
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("os", &platform.os)
            .append_pair("arch", &platform.arch);
        if let Some(libc) = &platform.libc {
            q.append_pair("libc", libc);
        }
    }
    let mut request = http(RESOLVE_TIMEOUT)?.get(url.clone());
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|e| RegistryError::Unreachable {
        url: registry.to_owned(),
        reason: describe(&e),
    })?;
    let target = match &platform.libc {
        Some(libc) => format!("{}-{}-{libc}", platform.os, platform.arch),
        None => format!("{}-{}", platform.os, platform.arch),
    };
    match response.status() {
        s if s.is_success() => response
            .json()
            .map_err(|e| RegistryError::BadResponse(e.to_string()).into()),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(RegistryError::Unauthorized {
            status: response.status().as_u16(),
        }
        .into()),
        StatusCode::NOT_FOUND => Err(RegistryError::NotPublished { target }.into()),
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            Err(RegistryError::Unreachable {
                url: registry.to_owned(),
                reason: format!("HTTP {}", response.status().as_u16()),
            }
            .into())
        }
        s => Err(RegistryError::Status {
            url: url.to_string(),
            status: s.as_u16(),
        }
        .into()),
    }
}

/// The artifact's bytes, checked against the registry's size and sha256.
fn download(registry: &str, resolved: &Resolved) -> Result<Vec<u8>> {
    // Absolute normally; a relative URL resolves against the registry.
    let url = base(registry)?
        .join(&resolved.download_url)
        .map_err(|e| RegistryError::BadResponse(format!("download_url: {e}")))?;
    let unreachable = |e: reqwest::Error| RegistryError::Unreachable {
        url: registry.to_owned(),
        reason: describe(&e),
    };
    let response = http(DOWNLOAD_TIMEOUT)?
        .get(url)
        .send()
        .map_err(unreachable)?;
    if !response.status().is_success() {
        return Err(RegistryError::Status {
            url: resolved.download_url.clone(),
            status: response.status().as_u16(),
        }
        .into());
    }
    let bytes = response.bytes().map_err(unreachable)?.to_vec();
    if let Some(size) = resolved.file_size
        && size != bytes.len() as u64
    {
        return Err(RegistryError::Mismatch(format!(
            "{} is {} bytes, expected {size}",
            resolved.file_name,
            bytes.len()
        ))
        .into());
    }
    let digest = hex::encode(Sha256::digest(&bytes));
    if !digest.eq_ignore_ascii_case(resolved.sha256.trim()) {
        return Err(RegistryError::Mismatch(format!(
            "{} has sha256 {digest}, expected {}",
            resolved.file_name, resolved.sha256
        ))
        .into());
    }
    Ok(bytes)
}

/// The binary inside the artifact.
#[allow(clippy::case_sensitive_file_extension_comparisons)] // lower-cased first
fn extract(resolved: &Resolved, bytes: Vec<u8>) -> Result<Vec<u8>> {
    let name = resolved.file_name.to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let bin = resolved.bin_name.as_deref().unwrap_or(BIN_NAME);
        return from_tar_gz(&bytes, bin);
    }
    if [".zip", ".tar.zst", ".tzst", ".tar.xz", ".tar"]
        .iter()
        .any(|ext| name.ends_with(ext))
    {
        return Err(RegistryError::BadResponse(format!(
            "{} is an archive format self-update does not unpack; rerun the installer",
            resolved.file_name
        ))
        .into());
    }
    Ok(bytes)
}

/// The regular file named `bin` anywhere in the archive, as the installer looks for it.
fn from_tar_gz(bytes: &[u8], bin: &str) -> Result<Vec<u8>> {
    let bad = |e: std::io::Error| RegistryError::BadResponse(format!("artifact: {e}"));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    for entry in archive.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let is_bin = entry.header().entry_type().is_file()
            && entry
                .path()
                .ok()
                .and_then(|p| p.file_name().map(|n| n == bin))
                .unwrap_or(false);
        if is_bin {
            let mut out = Vec::new();
            entry.read_to_end(&mut out).map_err(bad)?;
            return Ok(out);
        }
    }
    Err(RegistryError::BadResponse(format!("artifact has no `{bin}`")).into())
}

/// Swap `exe` for `binary`: a temp file in the same directory (same filesystem), made
/// executable, renamed over it.
fn replace(exe: &Path, binary: &[u8]) -> Result<()> {
    let dir = exe.parent().unwrap_or(Path::new("."));
    let name = exe
        .file_name()
        .map_or_else(|| BIN_NAME.into(), |n| n.to_string_lossy().into_owned());
    let tmp: PathBuf = dir.join(format!(".{name}.new.{}", std::process::id()));
    let result = (|| {
        std::fs::write(&tmp, binary).map_err(|e| Error::io(&tmp, e))?;
        set_executable(&tmp)?;
        std::fs::rename(&tmp, exe).map_err(|e| Error::io(exe, e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| Error::io(path, e))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(file_name: &str) -> Resolved {
        Resolved {
            version: "1.0.0".into(),
            target: "linux-amd64".into(),
            download_url: "/d".into(),
            file_name: file_name.into(),
            sha256: String::new(),
            file_size: None,
            bin_name: None,
        }
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.1.10", "0.1.9").unwrap());
        assert!(!is_newer("0.1.9", "0.1.9").unwrap());
        assert!(!is_newer("0.1.9", "0.2.0").unwrap());
        assert!(is_newer("0.0.1", "dev").unwrap());
        assert!(is_newer("garbage", "0.1.0").is_err());
    }

    #[test]
    fn raw_and_unsupported_artifacts() {
        assert_eq!(
            extract(&resolved("standards"), b"bin".to_vec()).unwrap(),
            b"bin"
        );
        for name in ["x.zip", "x.tar.zst", "x.TAR.XZ"] {
            let e = extract(&resolved(name), Vec::new()).unwrap_err();
            assert!(e.to_string().contains("does not unpack"), "{name}");
        }
        let e = extract(&resolved("x.tar.gz"), b"not gzip".to_vec()).unwrap_err();
        assert!(e.to_string().contains("artifact"), "{e}");
    }

    #[test]
    fn bad_registry_urls() {
        let p = Platform::current();
        for bad in ["ftp://x", "nope"] {
            let e = resolve(bad, &p, None).unwrap_err();
            assert_eq!(e.key(), "dev.thmsn.standards.cli.registry.error.bad_url");
        }
    }

    #[test]
    fn replace_failure_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        // Renaming a file over a non-empty directory fails.
        let exe = dir.path().join("standards");
        std::fs::create_dir(&exe).unwrap();
        std::fs::write(exe.join("x"), "").unwrap();
        assert!(replace(&exe, b"new").is_err());
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(left.len(), 1);
    }
}
