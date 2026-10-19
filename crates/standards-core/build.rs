// Version-deriving build script (templates/rust/build.rs) plus libbuildinfo.
//
// Emits `APP_VERSION` for `standards_core::VERSION`. Server and cli are built from the
// same commit as the content they serve, so they share this one derived version.
//
// Source of truth order:
//   1. `RELEASE_VERSION` env var (set by CI): authoritative.
//   2. Derived from git: `<tag MAJOR>.<tag MINOR>.<commit count>`, for local builds.
//   3. `0.0.0` fallback.
//
// `libbuildinfo::emit()` embeds commit hash, dirty flag and build host. Without a
// repository (an image build whose context has no `.git`) it fails; that is a warning,
// not an error, and `standards_core::build_info()` then returns `None`.
// See docs/versioning.md and docs/build-info.md.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RELEASE_VERSION");
    // The cfg `load_build_info!(optional)` tests for; declared here so its absence is a
    // known cfg rather than an unexpected one.
    println!("cargo:rustc-check-cfg=cfg(emitted_buildinfo)");
    emit_git_rerun_paths();

    if let Err(error) = libbuildinfo::emit() {
        println!("cargo:warning=build info not embedded: {error}");
    }

    let version = std::env::var("RELEASE_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(derive_version)
        .unwrap_or_else(|| "0.0.0".to_owned());

    println!("cargo:rustc-env=APP_VERSION={version}");
}

/// Watch the real git metadata paths.
///
/// `cargo:rerun-if-changed` resolves relative paths against the **package** root, and in a
/// workspace the package root has no `.git`, so a literal `.git/HEAD` names a file that does
/// not exist and reruns this script on every build. Ask git where its directory is instead.
fn emit_git_rerun_paths() {
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    if let Some(dir) = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]) {
        println!("cargo:rerun-if-changed={dir}/refs/tags");
        println!("cargo:rerun-if-changed={dir}/packed-refs");
    }
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
