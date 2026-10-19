//! The served content of this repo, embedded at build time.
//!
//! The server has no database: its "persistence" is the commit it was built from. This
//! crate embeds the served allowlist (L5: `docs/**`, `skills/**`, `templates/**`,
//! `CHANGELOG.md`) into the binary and hands it to [`standards_core::Bundle::from_files`]
//! once, with the commit the bytes came from.
//!
//! Each served directory is its own `RustEmbed` folder rather than one folder at the repo
//! root: rust-embed walks its whole folder before applying include/exclude patterns, and the
//! root holds `target/` and `.git`. `CHANGELOG.md` is a single file, so it is
//! `include_bytes!`. Dotfiles are excluded by pattern, and every path is checked again
//! with [`standards_core::is_served`] before it reaches the bundle.

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

use rust_embed::RustEmbed;
use standards_core::{Bundle, is_served};

pub use standards_core::{Error, Result};

#[derive(RustEmbed)]
#[folder = "../../docs"]
#[prefix = "docs/"]
#[exclude = ".*"]
#[exclude = "**/.*"]
struct Docs;

#[derive(RustEmbed)]
#[folder = "../../skills"]
#[prefix = "skills/"]
#[exclude = ".*"]
#[exclude = "**/.*"]
struct Skills;

#[derive(RustEmbed)]
#[folder = "../../templates"]
#[prefix = "templates/"]
#[exclude = ".*"]
#[exclude = "**/.*"]
struct Templates;

const CHANGELOG: &[u8] = include_bytes!("../../../CHANGELOG.md");

/// Commit placeholder when the build had neither `RELEASE_COMMIT` nor a repository.
pub const UNKNOWN_COMMIT: &str = "unknown";

/// Every embedded file before the allowlist guard: `(repo-relative path, bytes)`.
fn embedded() -> impl Iterator<Item = (Cow<'static, str>, Cow<'static, [u8]>)> {
    fn from<E: RustEmbed>() -> impl Iterator<Item = (Cow<'static, str>, Cow<'static, [u8]>)> {
        E::iter().filter_map(|path| E::get(&path).map(|file| (path, file.data)))
    }
    from::<Docs>()
        .chain(from::<Skills>())
        .chain(from::<Templates>())
        .chain(std::iter::once((
            Cow::Borrowed("CHANGELOG.md"),
            Cow::Borrowed(CHANGELOG),
        )))
}

/// The served files, `(repo-relative path, bytes)`, sorted by path. Anything the embed
/// patterns let through that [`is_served`] rejects is dropped here.
#[must_use]
pub fn files() -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<_> = embedded()
        .filter(|(path, _)| is_served(path))
        .map(|(path, bytes)| (path.into_owned(), bytes.into_owned()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The commit the embedded content came from: the content's identity.
///
/// `RELEASE_COMMIT` at build time (CI sets it), else the commit libbuildinfo recorded for
/// this build (the same one the derived version counts to), else [`UNKNOWN_COMMIT`]. A
/// local build from a dirty tree reports its HEAD commit.
#[must_use]
pub fn commit() -> &'static str {
    static COMMIT: OnceLock<String> = OnceLock::new();
    COMMIT.get_or_init(|| {
        resolve_commit(
            option_env!("RELEASE_COMMIT"),
            standards_core::build_info().map(|bi| bi.git.commit_hash),
        )
    })
}

fn resolve_commit(release: Option<&str>, built_from: Option<String>) -> String {
    release
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
        .or(built_from.filter(|c| !c.is_empty()))
        .unwrap_or_else(|| UNKNOWN_COMMIT.to_owned())
}

/// The embedded content as a [`Bundle`], parsed on first call and shared afterwards.
///
/// An error means the embedded content does not parse; the repo-content tests catch that
/// before a build ships, so the server treats it as fatal at startup.
pub fn bundle() -> Result<Arc<Bundle>> {
    static BUNDLE: OnceLock<Result<Arc<Bundle>>> = OnceLock::new();
    BUNDLE
        .get_or_init(|| Bundle::from_files(files(), commit()).map(Arc::new))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embed patterns alone already match the allowlist: the `is_served` guard in
    /// [`files`] drops nothing, so the two can't silently drift apart.
    #[test]
    fn embed_patterns_match_the_allowlist() {
        let raw: Vec<String> = embedded().map(|(p, _)| p.into_owned()).collect();
        let rejected: Vec<&String> = raw.iter().filter(|p| !is_served(p)).collect();
        assert!(rejected.is_empty(), "embedded but not served: {rejected:?}");
        assert_eq!(raw.len(), files().len());
    }

    #[test]
    fn commit_source_order() {
        assert_eq!(resolve_commit(Some("abc"), Some("def".into())), "abc");
        assert_eq!(resolve_commit(Some(" abc\n"), None), "abc");
        assert_eq!(resolve_commit(Some(""), Some("def".into())), "def");
        assert_eq!(resolve_commit(None, Some("def".into())), "def");
        assert_eq!(resolve_commit(None, Some(String::new())), UNKNOWN_COMMIT);
        assert_eq!(resolve_commit(None, None), UNKNOWN_COMMIT);
    }
}
