//! The page shell and each page's markup. Plain `format!`: the pages are few and small.

use std::fmt::Write as _;

use standards_core::{Applicability, Archetype, StandardRef};
use standards_engine::{DocView, Hit, HitKind};

use super::render::{self, esc};

/// What every page shares: the asset version (cache-busting) and the footer facts.
pub struct Shell {
    /// Appended to asset URLs so a deploy never serves a stale stylesheet.
    pub asset_version: String,
    pub version: String,
    pub commit: String,
}

const FAVICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='7' fill='%232e5e5a'/%3E%3Cpath d='M9 10h14M9 16h14M9 22h9' stroke='%23fff' stroke-width='2.6' stroke-linecap='round'/%3E%3C/svg%3E";

impl Shell {
    /// A whole page. `query` prefills the header search box.
    #[must_use]
    pub fn page(&self, title: &str, description: &str, query: &str, body: &str) -> String {
        let v = esc(&self.asset_version);
        let commit = esc(self.commit.get(..12).unwrap_or(&self.commit));
        format!(
            r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>{title}</title>
<meta name="description" content="{description}">
<link rel="icon" href="{FAVICON}">
<link rel="stylesheet" href="/static/site.css?v={v}">
<script src="/static/search.js?v={v}" defer></script>
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="top">
<div class="wrap top-in">
<a class="brand" href="/"><span class="mark" aria-hidden="true"></span>Standards</a>
<form class="search" action="/search" method="get" role="search">
<input type="search" name="q" value="{query}" placeholder="Search the standards" aria-label="Search the standards" autocomplete="off" spellcheck="false">
<ul class="live" role="listbox" aria-label="Results" hidden></ul>
</form>
</div>
</header>
<main id="main" class="wrap">
{body}
</main>
<footer class="wrap foot">
<span>Version {version}, commit <code>{commit}</code></span>
<span><a href="/api/docs">API</a> <a href="/manifest.json">Manifest</a> <a href="https://github.com/charliethomson/standards">Source</a></span>
</footer>
</body>
</html>
"##,
            title = esc(title),
            description = esc(description),
            query = esc(query),
            version = esc(&self.version),
        )
    }
}

/// A doc in a listing: id, title, blurb.
pub struct Entry {
    pub id: String,
    pub title: String,
    pub blurb: Option<String>,
}

/// A titled list of docs.
pub struct Section {
    pub title: String,
    pub entries: Vec<Entry>,
}

fn applicability(a: Applicability) -> (&'static str, &'static str) {
    match a {
        Applicability::Defining => ("defining", "Defining"),
        Applicability::Applies => ("applies", "Applies"),
        Applicability::Adapted => ("adapted", "Adapted"),
        Applicability::Optional => ("optional", "Optional"),
        Applicability::NotApplicable => ("na", "N/A"),
    }
}

/// A row's note without the leading bold marker the badge already says.
fn note_text(note: &str, a: Applicability) -> &str {
    if matches!(a, Applicability::Applies | Applicability::Optional) {
        return note;
    }
    let Some(rest) = note.trim_start().strip_prefix("**") else {
        return note;
    };
    let Some((_, after)) = rest.split_once("**") else {
        return note;
    };
    after.trim_start_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | ':' | ',' | '\u{2014}' | '\u{2013}' | '-')
    })
}

/// An archetype's "Standards that apply" table.
fn archetype_table(
    arch: &Archetype,
    title_of: &dyn Fn(&str) -> String,
    linker_note: &dyn Fn(&str) -> String,
) -> String {
    let mut rows = String::new();
    for row in &arch.rows {
        let names: Vec<String> = row
            .standards
            .iter()
            .map(|s| match s {
                StandardRef::Doc(id) => format!(
                    "<a href=\"/d/{}\">{}</a>",
                    esc(id.as_str()),
                    esc(&title_of(id.as_str()))
                ),
                StandardRef::Named(name) => esc(name),
            })
            .collect();
        let (class, label) = applicability(row.applicability);
        let note = linker_note(note_text(&row.note, row.applicability));
        let _ = writeln!(
            rows,
            "<tr class=\"a-{class}\"><td class=\"std-name\">{}</td><td><span class=\"tag tag-{class}\">{label}</span></td><td>{}</td></tr>",
            names.join(", "),
            render::inline(&note),
        );
    }
    format!(
        "<div class=\"scroll\"><table class=\"std\"><thead><tr><th>Standard</th><th>Applies</th><th>How it applies here</th></tr></thead><tbody>\n{rows}</tbody></table></div>"
    )
}

/// The index: archetypes with their tables, then every doc by group.
#[must_use]
pub fn index(
    archetypes: &[(String, &Archetype)],
    title_of: &dyn Fn(&str) -> String,
    note_html: &dyn Fn(&Archetype, &str) -> String,
    sections: &[Section],
) -> String {
    let mut out = String::from(
        "<section class=\"hero\">\n<h1>Standards</h1>\n<p class=\"lede\">How this fleet of repos is built. Find your repo's archetype, then follow the standards it applies.</p>\n</section>\n",
    );

    out.push_str("<section class=\"block\" aria-labelledby=\"archetypes\">\n<h2 id=\"archetypes\">Archetypes</h2>\n<p class=\"sub\">A repo is exactly one of these. Open one to see how each standard applies to it.</p>\n");
    for (pitch, arch) in archetypes {
        let name = arch
            .title
            .strip_prefix("Archetype: ")
            .unwrap_or(&arch.title);
        let applies = arch
            .rows
            .iter()
            .filter(|r| r.applicability != Applicability::NotApplicable)
            .count();
        let note = |n: &str| note_html(arch, n);
        let _ = write!(
            out,
            "<details class=\"arch\" id=\"a-{id}\">\n<summary><span class=\"arch-name\">{name}</span><span class=\"arch-count\">{applies} of {total} apply</span><span class=\"arch-pitch\">{pitch}</span></summary>\n<div class=\"arch-body\">\n{table}\n<p class=\"more\"><a href=\"/d/{doc}\">Read the {name} archetype</a></p>\n</div>\n</details>\n",
            id = esc(&arch.name),
            name = esc(name),
            total = arch.rows.len(),
            table = archetype_table(arch, title_of, &note),
            doc = esc(arch.doc.as_str()),
        );
    }
    out.push_str("</section>\n");

    out.push_str(
        "<section class=\"block\" aria-labelledby=\"docs\">\n<h2 id=\"docs\">All docs</h2>\n",
    );
    for section in sections {
        let _ = write!(
            out,
            "<h3 class=\"group\">{}</h3>\n<ul class=\"doclist\">\n",
            esc(&section.title)
        );
        for e in &section.entries {
            let blurb = e
                .blurb
                .as_deref()
                .map(|b| format!("<span class=\"blurb\">{b}</span>"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "<li><a href=\"/d/{}\"><span class=\"name\">{}</span>{blurb}</a></li>",
                esc(&e.id),
                esc(&e.title),
            );
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</section>\n");
    out
}

/// A neighbour in the doc order, for the pager.
pub struct Link<'a> {
    pub id: &'a str,
    pub title: &'a str,
}

/// A doc page.
#[must_use]
pub fn doc(
    view: &DocView,
    group: Option<&str>,
    prev: Option<Link<'_>>,
    next: Option<Link<'_>>,
) -> String {
    let anchors: Vec<String> = view.sections.iter().map(|s| s.anchor.clone()).collect();
    let body = render::doc_body(&view.markdown, &anchors);
    let toc_items = render::toc(
        &view
            .sections
            .iter()
            .map(|s| (s.level, s.title.clone(), s.anchor.clone()))
            .collect::<Vec<_>>(),
    );
    let crumb = group
        .map(|g| {
            format!(
                " <span aria-hidden=\"true\">/</span> <span>{}</span>",
                esc(g)
            )
        })
        .unwrap_or_default();
    let (toc_side, toc_inline) = if toc_items.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!(
                "<aside class=\"toc-side\"><nav aria-label=\"On this page\"><p class=\"toc-h\">On this page</p><ol class=\"toc\">{toc_items}</ol></nav></aside>"
            ),
            format!(
                "<details class=\"toc-inline\"><summary>On this page</summary><ol class=\"toc\">{toc_items}</ol></details>"
            ),
        )
    };
    let pager_link = |l: Option<Link<'_>>, rel: &str, label: &str| {
        l.map_or_else(|| "<span></span>".to_owned(), |l| {
            format!(
                "<a class=\"{rel}\" rel=\"{rel}\" href=\"/d/{}\"><span class=\"dir\">{label}</span><span class=\"name\">{}</span></a>",
                esc(l.id),
                esc(l.title)
            )
        })
    };
    format!(
        "<div class=\"doc-layout\">\n{toc_side}\n<article class=\"doc\">\n<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><a href=\"/\">All docs</a>{crumb}</nav>\n<h1>{title}</h1>\n{toc_inline}\n<div class=\"prose\">\n{body}</div>\n<nav class=\"pager\" aria-label=\"More docs\">{prev}{next}</nav>\n</article>\n</div>",
        title = esc(&view.title),
        prev = pager_link(prev, "prev", "Previous"),
        next = pager_link(next, "next", "Next"),
    )
}

/// The no-JS search results page.
#[must_use]
pub fn search(query: &str, hits: &[Hit]) -> String {
    let mut out = String::from("<div class=\"narrow\">\n<h1>Search</h1>\n");
    let _ = writeln!(
        out,
        "<form class=\"search-page\" action=\"/search\" method=\"get\" role=\"search\"><input type=\"search\" name=\"q\" value=\"{}\" aria-label=\"Search the standards\" placeholder=\"Search the standards\"><button type=\"submit\">Search</button></form>",
        esc(query)
    );
    let docs: Vec<&Hit> = hits.iter().filter(|h| h.kind == HitKind::Doc).collect();
    if query.trim().is_empty() {
        out.push_str("<p class=\"sub\">Search titles, sections and checklist items.</p>\n");
    } else if docs.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"sub\">No results for {}.</p>",
            quoted(query)
        );
    } else {
        let _ = writeln!(
            out,
            "<p class=\"sub\">{} {} for {}.</p>",
            docs.len(),
            if docs.len() == 1 { "result" } else { "results" },
            quoted(query)
        );
        out.push_str("<ol class=\"results\">\n");
        for h in docs {
            let href = match &h.anchor {
                Some(a) => format!("/d/{}#{}", h.doc, a),
                None => format!("/d/{}", h.doc),
            };
            let _ = writeln!(
                out,
                "<li><a href=\"{}\"><span class=\"name\">{}</span><span class=\"snippet\">{}</span></a></li>",
                esc(&href),
                esc(&h.title),
                esc(&h.snippet)
            );
        }
        out.push_str("</ol>\n");
    }
    out.push_str("</div>\n");
    out
}

fn quoted(q: &str) -> String {
    format!("<q>{}</q>", esc(q.trim()))
}

/// The page for an unknown doc id.
#[must_use]
pub fn not_found(id: &str) -> String {
    format!(
        "<div class=\"narrow\">\n<h1>No such doc</h1>\n<p class=\"sub\">There is no doc called <code>{}</code>.</p>\n<p><a href=\"/\">All docs</a> or <a href=\"/search?q={}\">search for it</a>.</p>\n</div>\n",
        esc(id),
        esc(&query_escape(id.rsplit('/').next().unwrap_or(id)))
    )
}

/// Percent-encode a query value (everything but unreserved characters).
fn query_escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}
