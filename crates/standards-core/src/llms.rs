//! What the whole-corpus routes serve: `/llms.txt` (the index) and `/llms-full.txt`.
//!
//! Both are composed here, from a [`Bundle`] and nothing else, so the order the site's
//! index lists docs in and the order these files list them in are one rule ([`outline`])
//! instead of two that drift. The server hands the strings back and adds the headers
//! (`standards-api`'s `lib.rs`); `web.rs` reads the same outline for its index.
//!
//! A doc's body is emitted exactly as `/d/<id>` and the REST mirror serve it, links
//! already absolute (L25), so nothing here re-derives what a doc says.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::ORIGIN;
use crate::archetype::Archetype;
use crate::bundle::Bundle;
use crate::doc::DocId;

/// The doc the order comes from, and the one that leads the trailing group.
const OVERVIEW: &str = "overview";

/// A doc id and the one-line rule the overview's table puts beside it.
type Row = (String, Option<String>);

/// One of the overview's doc tables: its heading, and the docs the table lists.
type TableGroup = (String, Vec<Row>);

/// The group the archetype docs are listed under, in every surface.
pub const ARCHETYPES_GROUP: &str = "Archetypes";

/// How long a summary on `/llms.txt` may be before it is cut (characters).
const SUMMARY_MAX: usize = 200;

/// The pitch both files open with, as the overview's own opening puts it.
const LEDE: &str = "\
> How this fleet of repos is built: versioning, identifiers, build info, CI, deployment,
> testing, service architecture, observability and the rest. Descriptive of patterns
> already in use across the reference repos, not aspirational.
";

/// One doc in the published order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    pub id: DocId,
    /// The doc's title, an archetype's `Archetype: ` prefix dropped.
    pub title: String,
    /// The overview table's one-line rule for this doc: inline markdown, links flattened to
    /// their text. `None` for a doc no table lists; `/llms.txt` falls back to the doc's
    /// first paragraph, the site's index prints nothing.
    pub summary: Option<String>,
}

/// A titled run of docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineGroup {
    pub title: String,
    pub entries: Vec<OutlineEntry>,
}

/// Every doc in the order the site publishes them, grouped:
///
/// 1. each of the overview's doc tables, in document order, named by its heading (rows
///    linking an archetype are skipped: they get their own group);
/// 2. `Archetypes`, in the order the overview's archetype table names them;
/// 3. `Reference`: everything left, the overview first, then by title.
///
/// The last group is the fallback, so a doc the overview does not link still appears, and
/// every doc in `bundle` appears exactly once.
#[must_use]
pub fn outline(bundle: &Bundle) -> Vec<OutlineGroup> {
    let overview = bundle
        .doc_markdown(&DocId::new(OVERVIEW))
        .unwrap_or_default();
    let titles: BTreeMap<&str, &str> = bundle
        .docs
        .iter()
        .map(|d| (d.id.as_str(), d.title.as_str()))
        .collect();

    let mut placed: BTreeSet<String> = BTreeSet::new();
    let mut groups: Vec<OutlineGroup> = Vec::new();

    for (title, rows) in overview_groups(&overview) {
        let entries: Vec<OutlineEntry> = rows
            .into_iter()
            .filter(|(id, _)| titles.contains_key(id.as_str()) && placed.insert(id.clone()))
            .map(|(id, summary)| OutlineEntry {
                title: entry_title(titles.get(id.as_str()).copied().unwrap_or(&id)),
                id: DocId::new(id),
                summary,
            })
            .collect();
        if !entries.is_empty() {
            groups.push(OutlineGroup { title, entries });
        }
    }

    let linked = linked_ids(&overview);
    let mut archetypes: Vec<&Archetype> = bundle.archetypes.iter().collect();
    archetypes.sort_by_key(|a| {
        (
            linked
                .iter()
                .position(|id| id == a.doc.as_str())
                .unwrap_or(usize::MAX),
            a.name.clone(),
        )
    });
    let entries: Vec<OutlineEntry> = archetypes
        .iter()
        .filter(|a| placed.insert(a.doc.as_str().to_owned()))
        .map(|a| OutlineEntry {
            id: a.doc.clone(),
            title: entry_title(&a.title),
            summary: None,
        })
        .collect();
    if !entries.is_empty() {
        groups.push(OutlineGroup {
            title: ARCHETYPES_GROUP.to_owned(),
            entries,
        });
    }

    let mut rest: Vec<OutlineEntry> = bundle
        .docs
        .iter()
        .filter(|d| placed.insert(d.id.as_str().to_owned()))
        .map(|d| OutlineEntry {
            id: d.id.clone(),
            title: entry_title(&d.title),
            summary: None,
        })
        .collect();
    // The overview leads: it is where a reader starts.
    rest.sort_by_key(|e| (e.id.as_str() != OVERVIEW, e.title.to_lowercase()));
    if !rest.is_empty() {
        groups.push(OutlineGroup {
            title: "Reference".to_owned(),
            entries: rest,
        });
    }
    groups
}

/// `/llms.txt`: the index form, one link list per outline group.
///
/// A doc's description is its overview rule, or its first paragraph when no table lists it,
/// or nothing at all. Nothing is invented and the title is never repeated as a description.
#[must_use]
pub fn llms_index(bundle: &Bundle) -> String {
    let mut out = String::with_capacity(8 * 1024);
    out.push_str("# Standards\n\n");
    out.push_str(LEDE);
    let _ = write!(
        out,
        "\nEvery doc is also served on its own at {ORIGIN}/d/<id>, and the whole corpus as one\n\
         file at {ORIGIN}/llms-full.txt. The skills and templates this server also serves are\n\
         tools for an agent in a session, not reading material, so neither file carries them.\n\
         Content commit {}.\n",
        bundle.commit
    );
    for group in outline(bundle) {
        let _ = write!(out, "\n## {}\n\n", group.title);
        for entry in group.entries {
            let url = doc_url(&entry.id);
            match entry.summary.or_else(|| first_paragraph(bundle, &entry.id)) {
                Some(summary) => {
                    let _ = writeln!(out, "- [{}]({url}): {summary}", entry.title);
                }
                None => {
                    let _ = writeln!(out, "- [{}]({url})", entry.title);
                }
            }
        }
    }
    out
}

/// `/llms-full.txt`: every doc, in outline order, each preceded by an HTML comment naming
/// its id and canonical URL.
///
/// The comment is the delimiter and the only machine contract: it is invisible when the
/// file is rendered and cannot be confused with a doc's own content the way a `---` rule or
/// a heading can.
#[must_use]
pub fn llms_full(bundle: &Bundle) -> String {
    let groups = outline(bundle);
    let count: usize = groups.iter().map(|g| g.entries.len()).sum();
    let mut out = String::with_capacity(400 * 1024);
    out.push_str("# Standards\n\n");
    out.push_str(LEDE);
    let _ = write!(
        out,
        "\n- Source: {ORIGIN}\n- Index: {ORIGIN}/llms.txt\n- Content commit: {}\n\
         - {count} docs, in the order the site lists them\n\n\
         Each doc below is preceded by an HTML comment naming its id and canonical URL.\n",
        bundle.commit
    );
    for entry in groups.iter().flat_map(|g| &g.entries) {
        let Some(markdown) = bundle.doc_markdown(&entry.id) else {
            continue;
        };
        let _ = write!(
            out,
            "\n<!-- standards:doc id=\"{id}\" url=\"{url}\" -->\n\n",
            id = entry.id,
            url = doc_url(&entry.id),
        );
        out.push_str(markdown.trim_end());
        out.push('\n');
    }
    out
}

fn doc_url(id: &DocId) -> String {
    format!("{ORIGIN}/d/{id}")
}

/// A listing's name for a doc: an archetype doc's title without its `Archetype: ` prefix.
fn entry_title(title: &str) -> String {
    title
        .strip_prefix("Archetype: ")
        .unwrap_or(title)
        .to_owned()
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

/// The doc id an absolute served link names (`<origin>/d/archetypes/cli-tool#x` →
/// `archetypes/cli-tool`).
fn doc_id_of(url: &str) -> Option<&str> {
    let rest = url.strip_prefix(ORIGIN)?.strip_prefix("/d/")?;
    let id = rest.split(['#', '?']).next().unwrap_or(rest);
    (!id.is_empty()).then_some(id)
}

/// A run of inline events as one line of markdown: links flattened to their text, code and
/// emphasis kept, whitespace collapsed.
fn inline_markdown<'a>(events: impl IntoIterator<Item = Event<'a>>) -> String {
    let mut out = String::new();
    for event in events {
        match event {
            Event::Text(t) => out.push_str(&t),
            Event::Code(t) => {
                let _ = write!(out, "`{t}`");
            }
            Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => out.push('*'),
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => out.push_str("**"),
            Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough) => {
                out.push_str("~~");
            }
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every doc id the markdown links, in order, duplicates dropped.
fn linked_ids(markdown: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        if let Event::Start(Tag::Link { dest_url, .. }) = event
            && let Some(id) = doc_id_of(&dest_url)
            && !out.iter().any(|o| o == id)
        {
            out.push(id.to_owned());
        }
    }
    out
}

/// The overview's doc tables as `(heading, [(doc id, one-line rule)])`: every heading whose
/// table rows link docs becomes a group, each row an entry with its last cell as the rule.
/// Rows linking an archetype are skipped; [`outline`] lists those apart.
fn overview_groups(markdown: &str) -> Vec<TableGroup> {
    let mut groups: Vec<TableGroup> = Vec::new();
    let mut heading = String::new();
    let mut in_heading = false;
    let mut current = String::new();
    let mut cells: Vec<Vec<Event<'_>>> = Vec::new();
    let mut in_row = false;

    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                current.clear();
                // "Operating context (read this first)" groups as "Operating context".
                current.push_str(heading.split(" (").next().unwrap_or(&heading).trim());
            }
            Event::Text(t) | Event::Code(t) if in_heading => heading.push_str(&t),
            Event::Start(Tag::TableRow) => {
                in_row = true;
                cells.clear();
            }
            Event::End(TagEnd::TableRow) => {
                in_row = false;
                if let Some(entry) = row_entry(&cells) {
                    match groups.iter_mut().find(|(title, _)| *title == current) {
                        Some((_, entries)) => entries.push(entry),
                        None => groups.push((current.clone(), vec![entry])),
                    }
                }
            }
            Event::Start(Tag::TableCell) if in_row => cells.push(Vec::new()),
            Event::End(TagEnd::TableCell) => {}
            other if in_row => {
                if let Some(cell) = cells.last_mut() {
                    cell.push(other);
                }
            }
            _ => {}
        }
    }
    groups
}

/// The doc a table row names, and the row's last cell as its one-line rule.
fn row_entry(cells: &[Vec<Event<'_>>]) -> Option<Row> {
    let (at, id) = cells.iter().enumerate().find_map(|(i, cell)| {
        cell.iter().find_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => doc_id_of(dest_url).map(|id| (i, id)),
            _ => None,
        })
    })?;
    if id.starts_with("archetypes/") {
        return None;
    }
    let summary = (at + 1 < cells.len())
        .then(|| inline_markdown(cells[cells.len() - 1].iter().cloned()))
        .filter(|s| !s.is_empty());
    Some((id.to_owned(), summary))
}

/// A doc's first paragraph after its title, as one shortened line of markdown.
fn first_paragraph(bundle: &Bundle, id: &DocId) -> Option<String> {
    let doc = bundle.doc(id.as_str())?;
    let mut parser = Parser::new_ext(&doc.markdown, options())
        .skip_while(|e| !matches!(e, Event::Start(Tag::Paragraph)))
        .skip(1);
    let body: Vec<Event<'_>> = parser
        .by_ref()
        .take_while(|e| !matches!(e, Event::End(TagEnd::Paragraph)))
        .collect();
    let line = inline_markdown(body);
    (!line.is_empty()).then(|| shorten(&line))
}

/// `s` cut to [`SUMMARY_MAX`] characters: at the last sentence end that fits, else at the
/// last word boundary with an ellipsis.
fn shorten(s: &str) -> String {
    if s.chars().count() <= SUMMARY_MAX {
        return s.to_owned();
    }
    let cut = s
        .char_indices()
        .nth(SUMMARY_MAX)
        .map_or(s.len(), |(i, _)| i);
    let head = &s[..cut];
    if let Some(end) = head.rfind(". ") {
        return head[..=end].trim_end().to_owned();
    }
    match head.rfind(char::is_whitespace) {
        Some(space) => format!("{}…", head[..space].trim_end()),
        None => format!("{head}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERVIEW_MD: &str = "\
# Overview & archetypes

These standards describe how this fleet of repos is built.

## Pick the archetype

| Archetype | What it is |
|---|---|
| **[Full-stack product](archetypes/full-stack-product.md)** | A server and its clients |
| **[Library](archetypes/library.md)** | A shared crate |

## The standards

| Topic | Doc | One-line rule |
|---|---|---|
| Versioning | [versioning.md](versioning.md) | Derived `MAJOR.MINOR.<count>`; the tag is the only knob. |
| Testing | [testing.md](testing.md) | See [the testing doc](testing.md) for the gate. |

### Implementation standards (services & clients)

| Topic | Doc | One-line rule |
|---|---|---|
| Service architecture | [service-architecture.md](service-architecture.md) | Layered `core→db→engine→api`. |
";

    const ARCHETYPE_MD: &str = "\
# Archetype: Full-stack product

A server and its clients.

| Standard | How it applies here |
|---|---|
| [Versioning](../versioning.md) | applies |
";

    fn bundle(extra: &[(&str, &str)]) -> Bundle {
        let mut files: Vec<(&str, Vec<u8>)> = vec![
            ("docs/overview.md", OVERVIEW_MD.as_bytes().to_vec()),
            (
                "docs/archetypes/full-stack-product.md",
                ARCHETYPE_MD.as_bytes().to_vec(),
            ),
            (
                "docs/archetypes/library.md",
                ARCHETYPE_MD
                    .replace("Full-stack product", "Library")
                    .into_bytes(),
            ),
            (
                "docs/versioning.md",
                b"# Versioning\n\nVersions are derived.\n".to_vec(),
            ),
            (
                "docs/testing.md",
                b"# Testing\n\nAim for 80% on services.\n".to_vec(),
            ),
            (
                "docs/service-architecture.md",
                b"# Service architecture\n\nFour crates.\n".to_vec(),
            ),
        ];
        files.extend(extra.iter().map(|(p, b)| (*p, b.as_bytes().to_vec())));
        Bundle::from_files(files, "c0ffee").expect("fixture parses")
    }

    fn ids(groups: &[OutlineGroup]) -> Vec<(String, Vec<String>)> {
        groups
            .iter()
            .map(|g| {
                (
                    g.title.clone(),
                    g.entries.iter().map(|e| e.id.to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn the_overview_sets_the_order_then_archetypes_then_the_rest() {
        let bundle = bundle(&[("docs/glossary.md", "# Glossary\n\nTerms.\n")]);
        let groups = outline(&bundle);
        assert_eq!(
            ids(&groups),
            vec![
                (
                    "The standards".to_owned(),
                    vec!["versioning".to_owned(), "testing".to_owned()]
                ),
                (
                    "Implementation standards".to_owned(),
                    vec!["service-architecture".to_owned()]
                ),
                (
                    "Archetypes".to_owned(),
                    vec![
                        "archetypes/full-stack-product".to_owned(),
                        "archetypes/library".to_owned()
                    ]
                ),
                (
                    "Reference".to_owned(),
                    vec!["overview".to_owned(), "glossary".to_owned()]
                ),
            ]
        );
    }

    /// A doc no table lists still appears, so adding one cannot drop it off either file.
    #[test]
    fn an_unlisted_doc_lands_in_reference() {
        let bundle = bundle(&[("docs/aardvark.md", "# Aardvark\n\nNew.\n")]);
        let reference = outline(&bundle)
            .into_iter()
            .find(|g| g.title == "Reference")
            .expect("reference group");
        assert_eq!(
            reference
                .entries
                .iter()
                .map(|e| e.id.to_string())
                .collect::<Vec<_>>(),
            // The overview leads; the rest by title.
            vec!["overview".to_owned(), "aardvark".to_owned()]
        );
    }

    #[test]
    fn every_doc_is_listed_exactly_once() {
        let bundle = bundle(&[("docs/glossary.md", "# Glossary\n\nTerms.\n")]);
        let mut listed: Vec<String> = outline(&bundle)
            .iter()
            .flat_map(|g| g.entries.iter().map(|e| e.id.to_string()))
            .collect();
        let before = listed.len();
        listed.sort();
        listed.dedup();
        assert_eq!(listed.len(), before, "a doc is listed twice");
        let mut all: Vec<String> = bundle.docs.iter().map(|d| d.id.to_string()).collect();
        all.sort();
        assert_eq!(listed, all);
    }

    #[test]
    fn the_summary_is_the_last_cell_with_links_flattened() {
        let bundle = bundle(&[]);
        let groups = outline(&bundle);
        let standards = &groups[0].entries;
        assert_eq!(
            standards[0].summary.as_deref(),
            Some("Derived `MAJOR.MINOR.<count>`; the tag is the only knob.")
        );
        // The cell links another doc: the text survives, the link does not.
        assert_eq!(
            standards[1].summary.as_deref(),
            Some("See the testing doc for the gate.")
        );
        // An archetype's title loses its prefix, and no table gives it a rule.
        let archetypes = &groups[2].entries;
        assert_eq!(archetypes[0].title, "Full-stack product");
        assert_eq!(archetypes[0].summary, None);
    }

    #[test]
    fn the_index_falls_back_to_the_first_paragraph() {
        let bundle = bundle(&[("docs/glossary.md", "# Glossary\n\nTerms used here.\n")]);
        let index = llms_index(&bundle);
        assert!(
            index.starts_with("# Standards\n\n> How this fleet"),
            "{index}"
        );
        assert!(index.contains(&format!(
            "- [Versioning]({ORIGIN}/d/versioning): Derived `MAJOR.MINOR.<count>`; the tag is the only knob.\n"
        )));
        // No table lists the glossary, so its own first paragraph describes it.
        assert!(index.contains(&format!(
            "- [Glossary]({ORIGIN}/d/glossary): Terms used here.\n"
        )));
        assert!(index.contains("\n## Archetypes\n"));
        assert!(index.contains("Content commit c0ffee."));
    }

    #[test]
    fn a_doc_with_no_prose_gets_no_description() {
        let bundle = bundle(&[("docs/bare.md", "# Bare\n")]);
        let index = llms_index(&bundle);
        assert!(
            index.contains(&format!("- [Bare]({ORIGIN}/d/bare)\n")),
            "{index}"
        );
    }

    #[test]
    fn the_full_file_delimits_every_doc_and_serves_absolute_links() {
        let bundle = bundle(&[]);
        let full = llms_full(&bundle);
        assert!(
            full.contains("- 6 docs, in the order the site lists them"),
            "{full}"
        );
        for id in [
            "versioning",
            "testing",
            "service-architecture",
            "archetypes/full-stack-product",
            "overview",
        ] {
            let header =
                format!("\n<!-- standards:doc id=\"{id}\" url=\"{ORIGIN}/d/{id}\" -->\n\n");
            assert_eq!(full.matches(&header).count(), 1, "{id} in {full}");
        }
        // Bodies come through as the other routes serve them: own title, links absolute.
        assert!(full.contains("# Versioning\n\nVersions are derived."));
        assert!(full.contains(&format!("[Versioning]({ORIGIN}/d/versioning)")));
        assert!(
            !full.contains("](versioning.md)"),
            "a relative link survived"
        );
        assert!(full.ends_with('\n'));
        // The order is the outline's, so the overview is last and versioning first.
        let first = full
            .find("standards:doc id=\"versioning\"")
            .expect("versioning");
        let last = full
            .find("standards:doc id=\"overview\"")
            .expect("overview");
        assert!(first < last);
    }

    #[test]
    fn shorten_cuts_at_a_sentence_then_a_word() {
        let long = format!("{} end.", "a".repeat(SUMMARY_MAX));
        assert_eq!(shorten("short."), "short.");
        assert_eq!(
            shorten(&format!("One sentence. {long}")),
            "One sentence.",
            "cuts at the sentence end"
        );
        let words = "word ".repeat(60);
        let cut = shorten(&words);
        assert!(cut.ends_with('…'), "{cut}");
        assert!(cut.chars().count() <= SUMMARY_MAX, "{cut}");
    }
}
