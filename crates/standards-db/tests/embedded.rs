//! What the binary embeds: exactly the served allowlist of this checkout, parsed.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use standards_core::is_served;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy();
            out.insert(rel.replace('\\', "/"));
        }
    }
}

/// The L5 allowlist, checked over every embedded path: nothing from mcp/, bin/, prompts/,
/// the root NOTES.md or any dotfile reaches the binary.
#[test]
fn only_the_allowlist_is_embedded() {
    let files = standards_db::files();
    assert!(!files.is_empty(), "empty");
    for (path, _) in &files {
        assert!(is_served(path), "{path}");
        let top = path.split('/').next().unwrap();
        assert!(
            matches!(top, "docs" | "skills" | "templates") || path == "CHANGELOG.md",
            "{path}"
        );
        for banned in ["mcp/", "bin/", "prompts/", "crates/", "target/"] {
            assert!(!path.starts_with(banned), "{path}");
        }
        assert_ne!(path, "NOTES.md");
        assert!(
            !path.split('/').any(|seg| seg.starts_with('.')),
            "dotfile embedded: {path}"
        );
    }
}

/// Every served file in the checkout is embedded, and nothing else is.
#[test]
fn embedded_set_equals_served_files_on_disk() {
    let root = repo_root();
    let mut on_disk = BTreeSet::new();
    for dir in ["docs", "skills", "templates"] {
        walk(&root.join(dir), &root, &mut on_disk);
    }
    on_disk.insert("CHANGELOG.md".to_owned());
    on_disk.retain(|p| is_served(p));

    let embedded: BTreeSet<String> = standards_db::files().into_iter().map(|(p, _)| p).collect();
    assert_eq!(embedded, on_disk);

    for (path, bytes) in standards_db::files() {
        assert_eq!(bytes, fs::read(root.join(&path)).unwrap(), "{path}");
    }
}

#[test]
fn bundle_parses_once_and_is_shared() {
    let a = standards_db::bundle().unwrap();
    let b = standards_db::bundle().unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.commit, standards_db::commit());
    assert!(a.doc("versioning").is_some());
    assert!(a.archetype("cli-tool").is_some());
    assert!(a.changelog.is_some());
    assert!(!a.templates.is_empty(), "empty");

    // L28: every non-SKILL.md file in a skill dir is an asset, nesting kept.
    let logs = a.skill("thmsn-logs").unwrap();
    assert!(logs.asset("scripts/obsq.sh").is_some());
    assert!(logs.assets.iter().any(|x| x.path.starts_with("queries/")));
    assert!(
        a.skill("thmsn-deep-review")
            .unwrap()
            .asset("assets/review.template.md")
            .is_some()
    );
}

/// Content identity: the commit is a full git sha unless CI pinned one explicitly.
#[test]
fn commit_is_a_sha() {
    let commit = standards_db::commit();
    if option_env!("RELEASE_COMMIT").is_none() {
        assert_eq!(commit.len(), 40, "{commit}");
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
    }
}
