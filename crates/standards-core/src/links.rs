//! Link resolution and rewriting (L25).
//!
//! Docs link to each other relatively (`versioning.md`, `../ci-cd.md#x`,
//! `archetypes/library.md`, `../templates/rust/build.rs`) and skills/templates link into
//! the submodule (`standards/docs/shortcuts.md`). Served over HTTP or MCP those break, so
//! [`Linker::rewrite`] points them at absolute URLs: docs at `/d/<id>#anchor`, template
//! files at `/t/<path>`, anything else in the repo at GitHub, pinned to the commit.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use crate::doc::DocId;
use crate::path::normalize;
use crate::{ORIGIN, REPOSITORY};

/// What a link destination points at, once resolved against the file it appears in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// A doc, by id.
    Doc { id: DocId, anchor: Option<String> },
    /// A path under `templates/` (relative to it). `dir` when the link ends in `/`.
    Template {
        path: String,
        anchor: Option<String>,
        dir: bool,
    },
    /// Any other repo path (`README.md`, `mcp/`, `skills/x/SKILL.md`); `""` is the root.
    Repo {
        path: String,
        anchor: Option<String>,
        dir: bool,
    },
    /// `#anchor` on the same page.
    Anchor(String),
    /// Has a scheme (`https:`, `mailto:`): left alone.
    External(String),
    /// Escapes the repo or is empty: left alone.
    Unresolved(String),
}

/// A markdown link: its text, its raw destination and what that resolves to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub text: String,
    pub href: String,
    pub target: Target,
}

/// Resolve `href` as written in the repo file `base` (a normalized repo-relative path).
///
/// A leading `standards/` (or bare `standards`) means the submodule root, the way skills
/// and templates written for consuming repos spell it; a leading `/` means the repo root.
#[must_use]
pub fn resolve_link(base: &str, href: &str) -> Target {
    let href_trimmed = href.trim();
    if href_trimmed.is_empty() {
        return Target::Unresolved(href.to_owned());
    }
    if let Some(anchor) = href_trimmed.strip_prefix('#') {
        return Target::Anchor(anchor.to_owned());
    }
    if has_scheme(href_trimmed) {
        return Target::External(href.to_owned());
    }
    let (path, anchor) = match href_trimmed.split_once('#') {
        Some((p, a)) => (p, Some(a.to_owned()).filter(|a| !a.is_empty())),
        None => (href_trimmed, None),
    };
    let dir = path.ends_with('/');

    let joined = if path == "standards" {
        String::new()
    } else if let Some(rest) = path.strip_prefix("standards/") {
        rest.to_owned()
    } else if let Some(rest) = path.strip_prefix('/') {
        rest.to_owned()
    } else {
        match base.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/{path}"),
            None => path.to_owned(),
        }
    };

    let normalized = if joined.split('/').all(|s| s.is_empty() || s == ".") {
        String::new()
    } else {
        match normalize(&joined) {
            Some(p) => p,
            None => return Target::Unresolved(href.to_owned()),
        }
    };

    if let Some(id) = DocId::from_path(&normalized) {
        return Target::Doc { id, anchor };
    }
    if let Some(rest) = normalized.strip_prefix("templates/") {
        return Target::Template {
            path: rest.to_owned(),
            anchor,
            dir,
        };
    }
    let dir = dir || normalized.is_empty() || normalized == "templates";
    Target::Repo {
        path: normalized,
        anchor,
        dir,
    }
}

fn has_scheme(href: &str) -> bool {
    let Some((scheme, _)) = href.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

impl Link {
    /// Every link in `markdown` (inline and reference style), resolved against the repo
    /// file `base`. Links inside code are not links.
    #[must_use]
    pub fn all(base: &str, markdown: &str) -> Vec<Self> {
        extract(base, markdown)
    }
}

pub(crate) fn extract(base: &str, markdown: &str) -> Vec<Link> {
    let mut out = Vec::new();
    let mut open: Vec<(String, String)> = Vec::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                open.push((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((href, text)) = open.pop() {
                    let target = resolve_link(base, &href);
                    out.push(Link { text, href, target });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, text)) = open.last_mut() {
                    text.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

/// Turns resolved targets into absolute URLs. Knows which docs and templates exist, so a
/// link to a missing doc or a template directory goes to GitHub instead of a 404.
#[derive(Debug, Clone, Default)]
pub struct Linker {
    pub commit: String,
    pub docs: BTreeSet<DocId>,
    /// Paths relative to `templates/`.
    pub templates: BTreeSet<String>,
}

impl Linker {
    /// The absolute URL for `target`, or `None` when the link should be left as written
    /// (same-page anchors, external and unresolvable links).
    #[must_use]
    pub fn href(&self, target: &Target) -> Option<String> {
        let with_anchor = |mut url: String, anchor: &Option<String>| {
            if let Some(a) = anchor {
                let _ = write!(url, "#{a}");
            }
            url
        };
        match target {
            Target::Doc { id, anchor } if self.docs.contains(id) => {
                Some(with_anchor(format!("{ORIGIN}/d/{id}"), anchor))
            }
            Target::Doc { id, anchor } => Some(with_anchor(self.github(&id.path(), false), anchor)),
            Target::Template { path, anchor, dir } if !dir && self.templates.contains(path) => {
                Some(with_anchor(format!("{ORIGIN}/t/{path}"), anchor))
            }
            Target::Template { path, anchor, dir } => {
                let full = format!("templates/{}", path.trim_end_matches('/'));
                Some(with_anchor(self.github(&full, *dir), anchor))
            }
            Target::Repo { path, anchor, dir } => {
                Some(with_anchor(self.github(path, *dir), anchor))
            }
            Target::Anchor(_) | Target::External(_) | Target::Unresolved(_) => None,
        }
    }

    fn github(&self, path: &str, dir: bool) -> String {
        let kind = if dir { "tree" } else { "blob" };
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            format!("{REPOSITORY}/tree/{}", self.commit)
        } else {
            format!("{REPOSITORY}/{kind}/{}/{path}", self.commit)
        }
    }

    /// `markdown` (from the repo file `base`) with every inline link and reference
    /// definition pointed at its absolute URL. Everything else is byte-for-byte unchanged.
    #[must_use]
    pub fn rewrite(&self, base: &str, markdown: &str) -> String {
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);
        for (_, def) in parser.reference_definitions().iter() {
            if let Some(dest) = def_dest(markdown, def.span.clone()) {
                self.push_edit(&mut edits, base, markdown, dest);
            }
        }
        for (event, range) in parser.into_offset_iter() {
            if let Event::Start(
                Tag::Link {
                    link_type: LinkType::Inline,
                    ..
                }
                | Tag::Image {
                    link_type: LinkType::Inline,
                    ..
                },
            ) = event
                && let Some(dest) = inline_dest(markdown, range)
            {
                self.push_edit(&mut edits, base, markdown, dest);
            }
        }
        edits.sort_by_key(|(r, _)| r.start);
        let mut out = String::with_capacity(markdown.len());
        let mut at = 0;
        for (range, url) in edits {
            if range.start < at {
                continue;
            }
            out.push_str(&markdown[at..range.start]);
            out.push_str(&url);
            at = range.end;
        }
        out.push_str(&markdown[at..]);
        out
    }

    fn push_edit(
        &self,
        edits: &mut Vec<(Range<usize>, String)>,
        base: &str,
        markdown: &str,
        dest: Range<usize>,
    ) {
        if let Some(url) = self.href(&resolve_link(base, &markdown[dest.clone()])) {
            edits.push((dest, url));
        }
    }
}

/// The destination's byte range inside an inline link `[text](dest "title")`.
fn inline_dest(markdown: &str, range: Range<usize>) -> Option<Range<usize>> {
    let src = &markdown[range.clone()];
    let body_end = src.strip_suffix(')')?.len();
    let open = src[..body_end].rfind("](")? + 2;
    dest_in(src, open, body_end).map(|r| range.start + r.start..range.start + r.end)
}

/// The destination's byte range inside a reference definition `[label]: dest "title"`.
fn def_dest(markdown: &str, span: Range<usize>) -> Option<Range<usize>> {
    let src = &markdown[span.clone()];
    let open = src.find("]:")? + 2;
    dest_in(src, open, src.len()).map(|r| span.start + r.start..span.start + r.end)
}

/// Skip whitespace from `from`, then take `<...>` contents or a run of non-whitespace.
fn dest_in(src: &str, from: usize, to: usize) -> Option<Range<usize>> {
    let rest = &src[from..to];
    let start = from + (rest.len() - rest.trim_start().len());
    let tail = &src[start..to];
    if let Some(inner) = tail.strip_prefix('<') {
        let len = inner.find('>')?;
        return Some(start + 1..start + 1 + len);
    }
    let len = tail.find(char::is_whitespace).unwrap_or(tail.len());
    (len > 0).then_some(start..start + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, anchor: Option<&str>) -> Target {
        Target::Doc {
            id: DocId::new(id),
            anchor: anchor.map(str::to_owned),
        }
    }

    #[test]
    fn resolves_every_form_in_the_repo() {
        let d = "docs/testing.md";
        let a = "docs/archetypes/cli-tool.md";
        assert_eq!(resolve_link(d, "ci-cd.md"), doc("ci-cd", None));
        assert_eq!(
            resolve_link(d, "tracing.md#client--server-contract"),
            doc("tracing", Some("client--server-contract"))
        );
        assert_eq!(
            resolve_link(d, "archetypes/library.md"),
            doc("archetypes/library", None)
        );
        assert_eq!(resolve_link(a, "../versioning.md"), doc("versioning", None));
        assert_eq!(
            resolve_link(a, "../identifiers.md#x"),
            doc("identifiers", Some("x"))
        );
        assert_eq!(
            resolve_link("templates/SHORTCUTS.md", "standards/docs/shortcuts.md"),
            doc("shortcuts", None)
        );
        assert_eq!(
            resolve_link("skills/x/SKILL.md", "../../docs/tracing.md"),
            doc("tracing", None)
        );
        assert_eq!(resolve_link(d, "/docs/workflow.md"), doc("workflow", None));
        assert_eq!(
            resolve_link(d, "../templates/rust/build.rs"),
            Target::Template {
                path: "rust/build.rs".into(),
                anchor: None,
                dir: false
            }
        );
        assert_eq!(
            resolve_link(d, "../templates/deploy/"),
            Target::Template {
                path: "deploy".into(),
                anchor: None,
                dir: true
            }
        );
        assert_eq!(
            resolve_link(d, "../README.md"),
            Target::Repo {
                path: "README.md".into(),
                anchor: None,
                dir: false
            }
        );
        assert_eq!(
            resolve_link(d, "../mcp/"),
            Target::Repo {
                path: "mcp".into(),
                anchor: None,
                dir: true
            }
        );
        assert_eq!(
            resolve_link("skills/README.md", "standards"),
            Target::Repo {
                path: String::new(),
                anchor: None,
                dir: true
            }
        );
        assert_eq!(
            resolve_link(d, "../templates"),
            Target::Repo {
                path: "templates".into(),
                anchor: None,
                dir: true
            }
        );
        assert_eq!(resolve_link(d, "#rule"), Target::Anchor("rule".into()));
        assert_eq!(
            resolve_link(d, "https://x.dev/a.md"),
            Target::External("https://x.dev/a.md".into())
        );
        assert_eq!(
            resolve_link(d, "mailto:a@b"),
            Target::External("mailto:a@b".into())
        );
        assert_eq!(
            resolve_link(d, "../../escape.md"),
            Target::Unresolved("../../escape.md".into())
        );
        assert_eq!(resolve_link(d, "  "), Target::Unresolved("  ".into()));
        assert_eq!(resolve_link("CHANGELOG.md", "docs/x.md#"), doc("x", None));
        // `1:2` has no valid scheme (it starts with a digit), so it is a relative path.
        assert_eq!(
            resolve_link(d, "1:2"),
            Target::Repo {
                path: "docs/1:2".into(),
                anchor: None,
                dir: false
            }
        );
    }

    fn linker() -> Linker {
        Linker {
            commit: "abc1234".into(),
            docs: [DocId::new("versioning"), DocId::new("archetypes/library")].into(),
            templates: ["rust/build.rs".to_owned()].into(),
        }
    }

    #[test]
    fn hrefs() {
        let l = linker();
        assert_eq!(
            l.href(&doc("versioning", Some("rule"))).unwrap(),
            "https://standards.thmsn.dev/d/versioning#rule"
        );
        assert_eq!(
            l.href(&doc("gone", None)).unwrap(),
            "https://github.com/charliethomson/standards/blob/abc1234/docs/gone.md"
        );
        assert_eq!(
            l.href(&Target::Template {
                path: "rust/build.rs".into(),
                anchor: None,
                dir: false
            })
            .unwrap(),
            "https://standards.thmsn.dev/t/rust/build.rs"
        );
        assert_eq!(
            l.href(&Target::Template {
                path: "deploy".into(),
                anchor: None,
                dir: true
            })
            .unwrap(),
            "https://github.com/charliethomson/standards/tree/abc1234/templates/deploy"
        );
        assert_eq!(
            l.href(&Target::Repo {
                path: String::new(),
                anchor: None,
                dir: true
            })
            .unwrap(),
            "https://github.com/charliethomson/standards/tree/abc1234"
        );
        assert_eq!(l.href(&Target::Anchor("x".into())), None);
    }

    #[test]
    fn rewrite_touches_only_destinations() {
        let md = "# T\n\nSee [v](../versioning.md#rule \"title\"), [lib](library.md), \
                  [b](<../../templates/rust/build.rs>), [r](#local), [x](https://e.com), \
                  ![img](../../README.md) and `[code](../versioning.md)`.\n\n\
                  ```md\n[fenced](../versioning.md)\n```\n\n[ref]: ../versioning.md \"t\"\n\nUse [it][ref].\n";
        let out = linker().rewrite("docs/archetypes/library.md", md);
        let want = "# T\n\nSee [v](https://standards.thmsn.dev/d/versioning#rule \"title\"), \
                    [lib](https://standards.thmsn.dev/d/archetypes/library), \
                    [b](<https://standards.thmsn.dev/t/rust/build.rs>), [r](#local), [x](https://e.com), \
                    ![img](https://github.com/charliethomson/standards/blob/abc1234/README.md) and `[code](../versioning.md)`.\n\n\
                    ```md\n[fenced](../versioning.md)\n```\n\n[ref]: https://standards.thmsn.dev/d/versioning \"t\"\n\nUse [it][ref].\n";
        assert_eq!(out, want);
    }

    #[test]
    fn extract_collects_text_and_targets() {
        let links = extract(
            "docs/x.md",
            "a [the `lib*` crates](lib-ecosystem.md) b [u](https://u.dev)",
        );
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, "the lib* crates");
        assert_eq!(links[0].target, doc("lib-ecosystem", None));
        assert_eq!(links[1].target, Target::External("https://u.dev".into()));
    }

    #[test]
    fn dest_scanning_edge_cases() {
        assert_eq!(inline_dest("[a]()", 0..5), None);
        assert_eq!(inline_dest("[a]", 0..3), None);
        assert_eq!(def_dest("[a]", 0..3), None);
        assert_eq!(dest_in("<unterminated", 0, 13), None);
    }
}
