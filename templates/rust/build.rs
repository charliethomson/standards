// Canonical version-deriving build script.
//
// Emits `APP_VERSION` for the crate to consume via `env!("APP_VERSION")`
// (e.g. clap's `#[command(version = env!("APP_VERSION"))]`).
//
// Source of truth order:
//   1. `RELEASE_VERSION` env var (set by CI) — authoritative.
//   2. Derived from git: `<tag MAJOR>.<tag MINOR>.<commit count>` — for local builds.
//   3. `0.0.0` fallback.
//
// To also embed full build metadata (commit hash, dirty flag, build host/time),
// add `libbuildinfo` and call `libbuildinfo::emit()` here. See standards/docs/build-info.md.

use std::process::Command;

fn main() {
    // Re-run when HEAD moves or a tag is pushed, so the version stays current.
    println!("cargo:rerun-if-env-changed=RELEASE_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    let version = std::env::var("RELEASE_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(derive_version)
        .unwrap_or_else(|| "0.0.0".to_owned());

    println!("cargo:rustc-env=APP_VERSION={version}");
}

fn derive_version() -> Option<String> {
    let count = git(&["rev-list", "--count", "HEAD"])?;
    let (major, minor) = git(&["describe", "--tags", "--abbrev=0", "--match", "v[0-9]*"])
        .and_then(|tag| {
            let tag = tag.strip_prefix('v').unwrap_or(&tag).to_owned();
            let mut parts = tag.split('.');
            Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
        })
        .unwrap_or_else(|| ("0".to_owned(), "0".to_owned()));
    Some(format!("{major}.{minor}.{count}"))
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}
