//! A doc's `## Checklist`: the last `##` section of every standard.

use serde::Serialize;

use crate::doc::DocId;
use crate::links::{Link, extract};

/// The checklist of one doc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checklist {
    pub doc: DocId,
    pub items: Vec<ChecklistItem>,
}

/// One `- [ ]` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecklistItem {
    /// The item's markdown on one line: the text after `- [ ] `, with its 6-space-indented
    /// continuation lines joined by single spaces.
    pub text: String,
    /// The links in `text`, resolved against the doc.
    pub links: Vec<Link>,
}

const CONTINUATION: &str = "      ";

impl Checklist {
    /// Parse the items out of the checklist section's markdown (heading included).
    #[must_use]
    pub fn parse(doc: &DocId, section: &str) -> Self {
        let mut texts: Vec<String> = Vec::new();
        let mut in_item = false;
        for line in section.lines() {
            if let Some(rest) = item_start(line) {
                texts.push(rest.trim().to_owned());
                in_item = true;
            } else if in_item && line.starts_with(CONTINUATION) && !line.trim().is_empty() {
                if let Some(text) = texts.last_mut() {
                    text.push(' ');
                    text.push_str(line.trim());
                }
            } else {
                in_item = false;
            }
        }
        let base = doc.path();
        let items = texts
            .into_iter()
            .map(|text| ChecklistItem {
                links: extract(&base, &text),
                text,
            })
            .collect();
        Self {
            doc: doc.clone(),
            items,
        }
    }
}

fn item_start(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- [")?;
    let mut chars = rest.chars();
    let mark = chars.next()?;
    let rest = chars.as_str().strip_prefix("] ")?;
    matches!(mark, ' ' | 'x' | 'X').then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::Target;

    #[test]
    fn items_with_continuations_and_links() {
        let md = "## Checklist\n\n\
                  - [ ] `build.rs` derives the version.\n\
                  - [ ] Root `README.md` leads with the install one-liner (and that re-running it\n      \
                  upgrades), then a tour ([`../readmes.md`](../readmes.md)).\n\
                  - [x] Done one.\n\
                  \x20 - nested bullet is not a continuation\n\
                  - not an item\n\
                  \n      stray indented line after a break\n";
        let c = Checklist::parse(&DocId::new("archetypes/cli-tool"), md);
        assert_eq!(c.doc, DocId::new("archetypes/cli-tool"));
        let texts: Vec<_> = c.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "`build.rs` derives the version.",
                "Root `README.md` leads with the install one-liner (and that re-running it upgrades), then a tour ([`../readmes.md`](../readmes.md)).",
                "Done one.",
            ]
        );
        assert_eq!(c.items[1].links.len(), 1);
        assert_eq!(
            c.items[1].links[0].target,
            Target::Doc {
                id: DocId::new("readmes"),
                anchor: None
            }
        );
        assert_eq!(c.items[0].links, Vec::<Link>::new());
    }

    #[test]
    fn item_marker_shapes() {
        assert_eq!(item_start("- [ ] a"), Some("a"));
        assert_eq!(item_start("- [X] a"), Some("a"));
        assert_eq!(item_start("- [?] a"), None);
        assert_eq!(item_start("- []"), None);
        assert_eq!(item_start("- ["), None);
    }
}
