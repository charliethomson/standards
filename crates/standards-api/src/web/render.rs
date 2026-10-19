//! Markdown to HTML for the site.
//!
//! The engine hands out markdown whose links are absolute to the production host (L25).
//! Every link to that host is made root-relative here, so the site works on any host
//! (localhost included). Headings get the anchors core computed (GitHub style), in order,
//! so `#anchor` links from other docs and from search land on the right heading.

use std::fmt::Write as _;

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

/// The host the engine's absolute links point at.
pub const PUBLIC_ORIGIN: &str = "https://standards.thmsn.dev";

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

/// HTML-escape text for an element body or a quoted attribute.
#[must_use]
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `https://standards.thmsn.dev/d/x#a` → `/d/x#a`; anything else unchanged.
#[must_use]
pub fn local_href(url: &str) -> &str {
    match url.strip_prefix(PUBLIC_ORIGIN) {
        Some("") => "/",
        Some(rest) if rest.starts_with('/') => rest,
        _ => url,
    }
}

/// The doc id a site-local href names (`/d/archetypes/cli-tool#x` → `archetypes/cli-tool`).
#[must_use]
pub fn doc_id_of(href: &str) -> Option<&str> {
    let rest = local_href(href).strip_prefix("/d/")?;
    let id = rest.split(['#', '?']).next().unwrap_or(rest);
    (!id.is_empty()).then_some(id)
}

/// Point a link event at its root-relative href.
fn localize(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let local = local_href(&dest_url);
            let dest_url = if local.len() == dest_url.len() {
                dest_url
            } else {
                CowStr::from(local.to_owned())
            };
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        other => other,
    }
}

fn level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// A doc's body as HTML. The first `#` heading is dropped (the page prints the title);
/// every heading below it takes the next of `anchors` as its id plus a self-link. Tables
/// sit in a scroll container; task-list items render as non-interactive boxes inside a
/// `ul.tasks`.
#[must_use]
pub fn doc_body(markdown: &str, anchors: &[String]) -> String {
    let mut anchors = anchors.iter();
    let mut events: Vec<Event<'_>> = Vec::new();
    let mut title_seen = false;
    let mut in_title = false;
    let mut open_heading: Vec<Option<String>> = Vec::new();
    // Indices of open lists and items, to tag task lists after the fact.
    let mut lists: Vec<usize> = Vec::new();
    let mut items: Vec<usize> = Vec::new();

    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) if !title_seen => in_title = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) if in_title => {
                in_title = false;
                title_seen = true;
            }
            _ if in_title => {}
            Event::Start(Tag::Heading { level, .. }) => {
                let n = level_num(level);
                // Core gives every heading but the title an anchor, in document order.
                let anchor = if n >= 2 { anchors.next() } else { None };
                if let Some(anchor) = anchor {
                    events.push(Event::Html(format!("<h{n} id=\"{}\">", esc(anchor)).into()));
                } else {
                    events.push(Event::Html(format!("<h{n}>").into()));
                }
                open_heading.push(anchor.cloned());
            }
            Event::End(TagEnd::Heading(level)) => {
                let n = level_num(level);
                let close = match open_heading.pop().flatten() {
                    Some(anchor) => format!(
                        "<a class=\"hash\" href=\"#{a}\" aria-label=\"Link to this section\">#</a></h{n}>\n",
                        a = esc(&anchor)
                    ),
                    None => format!("</h{n}>\n"),
                };
                events.push(Event::Html(close.into()));
            }
            Event::Start(Tag::Table(_)) => {
                events.push(Event::Html("<div class=\"scroll\">".into()));
                events.push(event);
            }
            Event::End(TagEnd::Table) => {
                events.push(event);
                events.push(Event::Html("</div>\n".into()));
            }
            Event::Start(Tag::List(_)) => {
                lists.push(events.len());
                events.push(event);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                events.push(event);
            }
            Event::Start(Tag::Item) => {
                items.push(events.len());
                events.push(event);
            }
            Event::End(TagEnd::Item) => {
                items.pop();
                events.push(event);
            }
            Event::TaskListMarker(done) => {
                if let Some(&i) = items.last() {
                    events[i] = Event::Html("<li class=\"task\">".into());
                }
                if let Some(&l) = lists.last()
                    && matches!(events[l], Event::Start(Tag::List(None)))
                {
                    events[l] = Event::Html("<ul class=\"tasks\">\n".into());
                }
                let class = if done { "box done" } else { "box" };
                events.push(Event::Html(
                    format!("<span class=\"{class}\" aria-hidden=\"true\"></span>").into(),
                ));
            }
            other => events.push(localize(other)),
        }
    }
    let mut out = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut out, events.into_iter());
    out
}

/// A fragment of inline markdown (a table cell) as HTML, without the wrapping `<p>`.
#[must_use]
pub fn inline(markdown: &str) -> String {
    let mut out = String::new();
    html::push_html(
        &mut out,
        Parser::new_ext(markdown, options())
            .filter(|e| {
                !matches!(
                    e,
                    Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph)
                )
            })
            .map(localize),
    );
    out.trim().to_owned()
}

/// The first paragraph after the title, as inline HTML: a doc's one-line pitch.
#[must_use]
pub fn first_paragraph(markdown: &str) -> Option<String> {
    let mut parser = Parser::new_ext(markdown, options())
        .skip_while(|e| !matches!(e, Event::Start(Tag::Paragraph)))
        .skip(1);
    let body: Vec<Event<'_>> = parser
        .by_ref()
        .take_while(|e| !matches!(e, Event::End(TagEnd::Paragraph)))
        .map(localize)
        .collect();
    if body.is_empty() {
        return None;
    }
    let mut out = String::new();
    html::push_html(&mut out, body.into_iter());
    Some(out.trim().to_owned())
}

/// Every doc id the markdown links, in order, duplicates dropped.
#[must_use]
pub fn linked_ids(markdown: &str) -> Vec<String> {
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

/// One group of docs on the index, as the overview's tables group them.
#[derive(Debug, Clone)]
pub struct Group {
    pub title: String,
    /// Doc id and its one-line rule (inline HTML), in table order.
    pub entries: Vec<(String, Option<String>)>,
}

/// The overview's standards tables as groups: each heading whose table rows link docs
/// becomes a group named by the heading (parenthetical dropped), each row an entry with
/// its last cell as the blurb. Archetype docs are skipped (the index lists them apart).
#[must_use]
pub fn overview_groups(markdown: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
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
                    match groups.iter_mut().find(|g| g.title == current) {
                        Some(g) => g.entries.push(entry),
                        None => groups.push(Group {
                            title: current.clone(),
                            entries: vec![entry],
                        }),
                    }
                }
            }
            Event::Start(Tag::TableCell) if in_row => cells.push(Vec::new()),
            Event::End(TagEnd::TableCell) => {}
            other if in_row => {
                if let Some(cell) = cells.last_mut() {
                    cell.push(localize(other));
                }
            }
            _ => {}
        }
    }
    groups
}

fn row_entry(cells: &[Vec<Event<'_>>]) -> Option<(String, Option<String>)> {
    let (at, id) = cells.iter().enumerate().find_map(|(i, cell)| {
        cell.iter().find_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => doc_id_of(dest_url).map(|id| (i, id)),
            _ => None,
        })
    })?;
    if id.starts_with("archetypes/") {
        return None;
    }
    let blurb = (at + 1 < cells.len()).then(|| {
        let mut out = String::new();
        html::push_html(&mut out, cells[cells.len() - 1].iter().cloned());
        out.trim().to_owned()
    });
    Some((id.to_owned(), blurb))
}

/// `<li>`s for a table of contents: `##` entries with their `###` nested.
#[must_use]
pub fn toc(sections: &[(u8, String, String)]) -> String {
    let mut out = String::new();
    let mut open_sub = false;
    for (level, title, anchor) in sections {
        match level {
            2 => {
                if open_sub {
                    out.push_str("</ol></li>");
                    open_sub = false;
                } else if !out.is_empty() {
                    out.push_str("</li>");
                }
                let _ = write!(out, "<li><a href=\"#{}\">{}</a>", esc(anchor), esc(title));
            }
            3 if !out.is_empty() => {
                if !open_sub {
                    out.push_str("<ol>");
                    open_sub = true;
                }
                let _ = write!(
                    out,
                    "<li><a href=\"#{}\">{}</a></li>",
                    esc(anchor),
                    esc(title)
                );
            }
            _ => {}
        }
    }
    if open_sub {
        out.push_str("</ol>");
    }
    if !out.is_empty() {
        out.push_str("</li>");
    }
    out
}
