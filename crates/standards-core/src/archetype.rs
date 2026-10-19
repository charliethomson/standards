//! Archetypes: `docs/archetypes/<name>.md` and their "Standards that apply" table.

use serde::Serialize;

use crate::doc::{Doc, DocId};
use crate::error::{Error, Result};
use crate::links::{Target, extract};

/// How a standard applies to an archetype, from the row's leading bold marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    /// No marker: the standard applies as written.
    Applies,
    /// `**The defining standard**`, `**The defining rule**`, `**Also defining**`.
    Defining,
    /// `**Adapted**`: applies, changed as the note says.
    Adapted,
    /// `**Default off.**`, `**Tooling only.**`, `**Only if ...**`: applies conditionally.
    Optional,
    /// `**N/A**`: does not apply; the note says why.
    NotApplicable,
}

impl Applicability {
    /// Read the marker at the start of a row's note.
    #[must_use]
    pub fn from_note(note: &str) -> Self {
        let Some(rest) = note.trim_start().strip_prefix("**") else {
            return Self::Applies;
        };
        let Some((marker, _)) = rest.split_once("**") else {
            return Self::Applies;
        };
        let marker = marker.trim().trim_end_matches(['.', ':']).to_lowercase();
        if marker.starts_with("n/a") {
            Self::NotApplicable
        } else if marker.contains("defining") {
            Self::Defining
        } else if marker.starts_with("adapted") {
            Self::Adapted
        } else if marker.starts_with("default off")
            || marker.starts_with("tooling only")
            || marker.starts_with("only if")
        {
            Self::Optional
        } else {
            Self::Applies
        }
    }
}

/// What a row's first cell names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StandardRef {
    /// A linked doc.
    Doc(DocId),
    /// A plain-text row that is not a doc (orchestration's "Secrets split").
    Named(String),
}

/// One row of the "Standards that apply" table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArchetypeRow {
    /// Normally one; a cell linking several docs yields several.
    pub standards: Vec<StandardRef>,
    pub applicability: Applicability,
    /// The "How it applies here" cell, markdown as written (marker included).
    pub note: String,
}

/// A parsed archetype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Archetype {
    /// `cli-tool`, `full-stack-product`, ...
    pub name: String,
    pub doc: DocId,
    pub title: String,
    pub rows: Vec<ArchetypeRow>,
}

impl Archetype {
    /// Parse the archetype from its already-parsed doc.
    pub fn from_doc(doc: &Doc) -> Result<Self> {
        let name = doc
            .id
            .as_str()
            .strip_prefix("archetypes/")
            .unwrap_or(doc.id.as_str())
            .to_owned();
        let rows = parse_table(&doc.id, &doc.markdown)?;
        Ok(Self {
            name,
            doc: doc.id.clone(),
            title: doc.title.clone(),
            rows,
        })
    }

    /// The row for a doc, if the table lists it.
    #[must_use]
    pub fn row(&self, doc: &DocId) -> Option<&ArchetypeRow> {
        self.rows.iter().find(|r| {
            r.standards
                .iter()
                .any(|s| matches!(s, StandardRef::Doc(d) if d == doc))
        })
    }
}

fn parse_table(id: &DocId, markdown: &str) -> Result<Vec<ArchetypeRow>> {
    let base = id.path();
    let mut table = markdown.lines().skip_while(|l| !is_header(l));
    if table.next().is_none() {
        return Err(Error::MissingStandardsTable { path: base });
    }
    let mut rows = Vec::new();
    for line in table.skip(1) {
        let line = line.trim();
        if !line.starts_with('|') {
            break;
        }
        let cells = split_row(line);
        let [first, note, ..] = cells.as_slice() else {
            continue;
        };
        let links = extract(&base, first);
        let standards = if links.is_empty() {
            vec![StandardRef::Named(first.clone())]
        } else {
            links
                .into_iter()
                .map(|l| match l.target {
                    Target::Doc { id, .. } => StandardRef::Doc(id),
                    _ => StandardRef::Named(l.text),
                })
                .collect()
        };
        rows.push(ArchetypeRow {
            standards,
            applicability: Applicability::from_note(note),
            note: note.clone(),
        });
    }
    Ok(rows)
}

fn is_header(line: &str) -> bool {
    let cells = split_row(line.trim());
    cells.len() >= 2 && cells[0] == "Standard" && cells[1] == "How it applies here"
}

/// Split a `| a | b |` row into trimmed cells. A `|` inside backticks or escaped as `\|`
/// does not split.
fn split_row(line: &str) -> Vec<String> {
    let Some(inner) = line.strip_prefix('|') else {
        return Vec::new();
    };
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut in_code = false;
    let mut escaped = false;
    for c in inner.chars() {
        match c {
            '|' if !in_code && !escaped => cells.push(std::mem::take(&mut cell).trim().to_owned()),
            _ => {
                if c == '`' && !escaped {
                    in_code = !in_code;
                }
                escaped = c == '\\' && !escaped;
                cell.push(c);
            }
        }
    }
    cells.push(cell.trim().to_owned());
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers() {
        use Applicability as A;
        for (note, want) in [
            ("Derived `MAJOR.MINOR`.", A::Applies),
            (
                "**The defining standard** — published as kind = cli.",
                A::Defining,
            ),
            ("**The defining rule** — see below.", A::Defining),
            (
                "**Also defining** — published to the registry.",
                A::Defining,
            ),
            ("**Adapted** — ship upstream's version.", A::Adapted),
            ("**Default off.** Placeholder version.", A::Optional),
            ("**Tooling only.** Never applied to the app.", A::Optional),
            (
                "**Only if it ships an artifact.** A pure crate needs none.",
                A::Optional,
            ),
            ("**N/A** (a CLI is not a hosted service).", A::NotApplicable),
            ("**N/A.**", A::NotApplicable),
            ("This **is** a `lib*`.", A::Applies),
            ("**Unknown marker** text", A::Applies),
            ("**unterminated", A::Applies),
        ] {
            assert_eq!(Applicability::from_note(note), want, "{note}");
        }
    }

    #[test]
    fn rows_parse() {
        let md = "# Archetype: Thing\n\n## Standards that apply\n\n\
                  | Standard | How it applies here |\n|---|---|\n\
                  | [Versioning](../versioning.md) | **Default off.** `a | b` stays one cell. |\n\
                  | Secrets split | **The defining rule** — see below. |\n\
                  | [A](../a.md), [B](../b.md#x) | Both. |\n\
                  | [Out](https://example.com) | External. |\n\
                  | lonely |\n\
                  \nAfter the table | not | a row |\n";
        let doc = Doc::parse(DocId::new("archetypes/thing"), md.to_owned()).unwrap();
        let a = Archetype::from_doc(&doc).unwrap();
        assert_eq!(a.name, "thing");
        assert_eq!(a.title, "Archetype: Thing");
        assert_eq!(a.rows.len(), 4);
        assert_eq!(
            a.rows[0].standards,
            [StandardRef::Doc(DocId::new("versioning"))]
        );
        assert_eq!(a.rows[0].applicability, Applicability::Optional);
        assert_eq!(a.rows[0].note, "**Default off.** `a | b` stays one cell.");
        assert_eq!(
            a.rows[1].standards,
            [StandardRef::Named("Secrets split".into())]
        );
        assert_eq!(a.rows[1].applicability, Applicability::Defining);
        assert_eq!(
            a.rows[2].standards,
            [
                StandardRef::Doc(DocId::new("a")),
                StandardRef::Doc(DocId::new("b"))
            ]
        );
        assert_eq!(a.rows[3].standards, [StandardRef::Named("Out".into())]);
        assert!(a.row(&DocId::new("b")).is_some());
        assert!(a.row(&DocId::new("nope")).is_none());
    }

    #[test]
    fn missing_table_is_an_error() {
        let doc = Doc::parse(DocId::new("archetypes/x"), "# X\n".to_owned()).unwrap();
        assert_eq!(
            Archetype::from_doc(&doc).unwrap_err(),
            Error::MissingStandardsTable {
                path: "docs/archetypes/x.md".into()
            }
        );
    }

    #[test]
    fn split_row_handles_escapes() {
        assert_eq!(split_row(r"| a \| b | c |"), [r"a \| b", "c"]);
        assert_eq!(split_row("no pipes"), Vec::<String>::new());
    }
}
