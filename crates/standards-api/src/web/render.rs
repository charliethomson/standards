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

/// Where a backslash has to go to stop `s` opening a block instead of reading as inline
/// text, if anywhere: the testing standard's rule starts `>80%`, which parses as a
/// blockquote on its own. `None` when the fragment is already inline.
///
/// A marker is escaped in front (`\>80%`), except an ordered list's, where only the
/// delimiter can carry it (`1\.`, since `\1` is not an escape at all).
fn block_marker(s: &str) -> Option<usize> {
    let mut rest = s.chars().skip(1);
    let marked = |next: Option<char>| matches!(next, Some(' ') | None);
    match s.chars().next() {
        Some('>' | '|') => Some(0),
        Some('#' | '-' | '+' | '*') => marked(rest.next()).then_some(0),
        Some(c) if c.is_ascii_digit() => {
            let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let mut after = s[digits..].chars();
            (matches!(after.next(), Some('.' | ')')) && marked(after.next())).then_some(digits)
        }
        _ => None,
    }
}

/// A fragment of inline markdown (a table cell, a doc's one-line rule) as HTML, without the
/// wrapping `<p>`. A fragment that would open a block is escaped first, so it renders as the
/// text it is.
#[must_use]
pub fn inline(markdown: &str) -> String {
    let trimmed = markdown.trim_start();
    let escaped = match block_marker(trimmed) {
        Some(at) => format!("{}\\{}", &trimmed[..at], &trimmed[at..]),
        None => markdown.to_owned(),
    };
    let mut out = String::new();
    html::push_html(
        &mut out,
        Parser::new_ext(&escaped, options())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-line fragment renders as text, not as the block its first character suggests.
    #[test]
    fn inline_keeps_a_block_looking_fragment_inline() {
        assert_eq!(inline(">80% on services"), "&gt;80% on services");
        assert_eq!(inline("- not a list"), "- not a list");
        assert_eq!(inline("1. not a list"), "1. not a list");
        assert_eq!(inline("`core` only"), "<code>core</code> only");
        assert_eq!(inline("**bold** stays"), "<strong>bold</strong> stays");
        assert_eq!(
            inline("[Versioning](https://standards.thmsn.dev/d/versioning)"),
            "<a href=\"/d/versioning\">Versioning</a>"
        );
    }
}
