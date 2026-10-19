//! Repo-relative paths and the served allowlist.

/// Normalize a repo-relative path: forward slashes, no `.` segments, no leading `./` or
/// `/`, `..` collapsed. `None` when it escapes the repo root or is empty.
#[must_use]
pub fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    (!out.is_empty()).then(|| out.join("/"))
}

/// Whether the server serves this repo-relative path (L5): `docs/**`, `skills/**`,
/// `templates/**` and `CHANGELOG.md`. Never dotfiles, at any depth.
///
/// The path must already be normalized; an unnormalized one (`docs/../bin/x`) is refused.
#[must_use]
pub fn is_served(path: &str) -> bool {
    if normalize(path).as_deref() != Some(path) {
        return false;
    }
    if path.split('/').any(|seg| seg.starts_with('.')) {
        return false;
    }
    if path == "CHANGELOG.md" {
        return true;
    }
    matches!(
        path.split_once('/'),
        Some(("docs" | "skills" | "templates", rest)) if !rest.is_empty()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_and_refuses_escape() {
        assert_eq!(normalize("./docs//x.md").as_deref(), Some("docs/x.md"));
        assert_eq!(
            normalize("docs/archetypes/../x.md").as_deref(),
            Some("docs/x.md")
        );
        assert_eq!(normalize("docs\\x.md").as_deref(), Some("docs/x.md"));
        assert_eq!(normalize("../x.md"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("."), None);
    }

    #[test]
    fn allowlist() {
        for ok in [
            "docs/versioning.md",
            "docs/archetypes/cli-tool.md",
            "skills/thmsn-deep-review/assets/review.template.md",
            "skills/README.md",
            "templates/rust/build.rs",
            "CHANGELOG.md",
        ] {
            assert!(is_served(ok), "{ok}");
        }
        for no in [
            "mcp/README.md",
            "bin/standards",
            "prompts/install.md",
            "NOTES.md",
            ".mcp.json",
            "AGENTS.md",
            "README.md",
            "docs",
            "docs/",
            "docs/.DS_Store",
            "templates/.env",
            "docs/../bin/standards",
            "/docs/x.md",
            "changelog.md",
        ] {
            assert!(!is_served(no), "{no}");
        }
    }
}
