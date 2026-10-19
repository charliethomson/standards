//! Docs: `docs/**/*.md`, identified by their path stem.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::checklist::Checklist;
use crate::error::{Error, Result};

/// A doc's id: its path under `docs/` without `.md` (`versioning`, `archetypes/cli-tool`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocId(String);

impl DocId {
    /// The id for a normalized repo-relative path, when it names a doc
    /// (`docs/x.md` → `x`, `docs/archetypes/x.md` → `archetypes/x`).
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        let stem = path.strip_prefix("docs/")?.strip_suffix(".md")?;
        let ok = !stem.is_empty()
            && stem
                .split('/')
                .all(|s| !s.is_empty() && !s.starts_with('.'));
        ok.then(|| Self(stem.to_owned()))
    }

    /// Wrap an id as given (`"versioning"`); no validation, for lookups.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The repo-relative path of the doc's source file.
    #[must_use]
    pub fn path(&self) -> String {
        format!("docs/{}.md", self.0)
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for DocId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// One heading below the title, and the span of markdown it owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// 2 for `##`, 3 for `###`, ...
    pub level: u8,
    pub title: String,
    /// GitHub-style anchor, deduplicated within the doc (`client--server-contract`).
    pub anchor: String,
    /// Byte range in [`Doc::markdown`]: from the heading to the next heading of the same or
    /// a higher level.
    pub range: Range<usize>,
}

/// A parsed doc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Doc {
    pub id: DocId,
    /// The first `#` heading.
    pub title: String,
    /// The source markdown, unchanged.
    pub markdown: String,
    pub sections: Vec<Section>,
    /// The last `##` section, when it is titled `Checklist`.
    pub checklist: Option<Checklist>,
}

impl Doc {
    /// Parse a doc from its markdown. `path` is used for the id and error messages.
    pub fn parse(id: DocId, markdown: String) -> Result<Self> {
        let headings = headings(&markdown);
        let title = headings
            .iter()
            .find(|h| h.level == 1)
            .map(|h| h.text.clone())
            .ok_or_else(|| Error::MissingTitle { path: id.path() })?;

        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut sections = Vec::new();
        for (i, h) in headings.iter().enumerate() {
            if h.level == 1 {
                continue;
            }
            let end = headings[i + 1..]
                .iter()
                .find(|n| n.level <= h.level)
                .map_or(markdown.len(), |n| n.start);
            sections.push(Section {
                level: h.level,
                title: h.text.clone(),
                anchor: dedup(&mut seen, slugify(&h.text)),
                range: h.start..end,
            });
        }

        let checklist = sections
            .iter()
            .rev()
            .find(|s| s.level == 2)
            .filter(|s| s.title.trim().eq_ignore_ascii_case("checklist"))
            .map(|s| Checklist::parse(&id, &markdown[s.range.clone()]));

        Ok(Self {
            id,
            title,
            markdown,
            sections,
            checklist,
        })
    }

    /// The section with this anchor.
    #[must_use]
    pub fn section(&self, anchor: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.anchor == anchor)
    }

    /// The markdown of the section with this anchor, heading included.
    #[must_use]
    pub fn section_markdown(&self, anchor: &str) -> Option<&str> {
        self.section(anchor)
            .map(|s| &self.markdown[s.range.clone()])
    }
}

struct Heading {
    level: u8,
    text: String,
    start: usize,
}

fn headings(markdown: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut current: Option<Heading> = None;
    for (event, range) in Parser::new_ext(markdown, Options::ENABLE_TABLES).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(Heading {
                    level: heading_level(level),
                    text: String::new(),
                    start: range.start,
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = current.take() {
                    out.push(Heading {
                        text: h.text.trim().to_owned(),
                        ..h
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(h) = current.as_mut() {
                    h.text.push_str(&t);
                }
            }
            _ => {}
        }
    }
    out
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// GitHub's heading anchor: lowercased, everything but letters, digits, `-`, `_` and
/// spaces dropped, spaces turned into `-`. `Client ↔ server contract` →
/// `client--server-contract`.
#[must_use]
pub fn slugify(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn dedup(seen: &mut HashMap<String, usize>, slug: String) -> String {
    let n = seen.entry(slug.clone()).or_insert(0);
    let out = if *n == 0 { slug } else { format!("{slug}-{n}") };
    *n += 1;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_from_paths() {
        assert_eq!(
            DocId::from_path("docs/versioning.md"),
            Some(DocId::new("versioning"))
        );
        assert_eq!(
            DocId::from_path("docs/archetypes/cli-tool.md").map(|d| d.path()),
            Some("docs/archetypes/cli-tool.md".to_owned())
        );
        assert_eq!(DocId::from_path("docs/.md"), None);
        assert_eq!(DocId::from_path("docs/img.png"), None);
        assert_eq!(DocId::from_path("templates/x.md"), None);
        assert_eq!(DocId::new("x").to_string(), "x");
        assert_eq!(DocId::new("x").as_ref(), "x");
    }

    #[test]
    fn slugs_match_github() {
        assert_eq!(
            slugify("Client ↔ server contract"),
            "client--server-contract"
        );
        assert_eq!(slugify("The `lib*` toolchain"), "the-lib-toolchain");
        assert_eq!(
            slugify("build.rs and local builds"),
            "buildrs-and-local-builds"
        );
        assert_eq!(
            slugify("Conventions & placeholders"),
            "conventions--placeholders"
        );
        assert_eq!(slugify("snake_case Names"), "snake_case-names");
    }

    #[test]
    fn sections_nest_and_dedup() {
        let md = "# T\n\nintro\n\n## A\n\na\n\n### Sub\n\ns\n\n## A\n\nagain\n\n## Checklist\n\n- [ ] x\n";
        let doc = Doc::parse(DocId::new("t"), md.to_owned()).unwrap();
        assert_eq!(doc.title, "T");
        let anchors: Vec<_> = doc.sections.iter().map(|s| s.anchor.as_str()).collect();
        assert_eq!(anchors, ["a", "sub", "a-1", "checklist"]);
        assert_eq!(
            doc.section_markdown("a").unwrap(),
            "## A\n\na\n\n### Sub\n\ns\n\n"
        );
        assert_eq!(doc.section_markdown("sub").unwrap(), "### Sub\n\ns\n\n");
        assert_eq!(doc.section_markdown("a-1").unwrap(), "## A\n\nagain\n\n");
        assert!(doc.section("nope").is_none());
        assert_eq!(doc.checklist.unwrap().items.len(), 1);
    }

    #[test]
    fn heading_in_code_fence_is_not_a_section() {
        let md = "# T\n\n```sh\n# not a heading\n## nor this\n```\n";
        let doc = Doc::parse(DocId::new("t"), md.to_owned()).unwrap();
        assert_eq!(doc.sections, Vec::<Section>::new());
    }

    #[test]
    fn checklist_only_when_last_h2() {
        let md = "# T\n\n## Checklist\n\n- [ ] x\n\n## After\n";
        let doc = Doc::parse(DocId::new("t"), md.to_owned()).unwrap();
        assert!(doc.checklist.is_none());
    }

    #[test]
    fn missing_title_is_an_error() {
        let err = Doc::parse(DocId::new("t"), "## only h2\n".to_owned()).unwrap_err();
        assert_eq!(
            err,
            Error::MissingTitle {
                path: "docs/t.md".to_owned()
            }
        );
        assert_eq!(err.to_string(), "docs/t.md: no `# ` title");
    }
}
