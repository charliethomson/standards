//! What the server does with the content: search, composed checklists, docs and sections
//! with absolute links, templates, skills and the manifest.
//!
//! [`Engine`] wraps one [`Bundle`] (from `standards-db` in the server) and is what the REST
//! and MCP layers call. It does no I/O; everything is computed from the bundle, and the
//! search index and the two whole-corpus files are built once in [`Engine::new`].

mod error;
mod search;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use standards_core::{
    Applicability, Archetype, Asset, Bundle, ChecklistItem, Doc, DocId, Linker, Manifest, Rendered,
    Skill, Template, normalize,
};

pub use error::{Error, Result};
pub use search::{Hit, HitKind, Index};

/// A doc in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocSummary {
    pub id: DocId,
    pub title: String,
    pub has_checklist: bool,
}

/// A heading in a doc's outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionInfo {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

/// A whole doc, links made absolute (L25).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocView {
    pub id: DocId,
    pub title: String,
    pub markdown: String,
    pub sections: Vec<SectionInfo>,
}

/// One section of a doc (heading and subsections included), links made absolute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionView {
    pub doc: DocId,
    pub anchor: String,
    pub level: u8,
    pub title: String,
    pub markdown: String,
}

/// The checklist for one standard in one archetype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposedChecklist {
    pub standard: DocId,
    /// Archetype name (`cli-tool`).
    pub archetype: String,
    /// From the archetype's "Standards that apply" row; [`Applicability::Applies`] when the
    /// table doesn't list the standard (the tables are not exhaustive).
    pub applicability: Applicability,
    /// Whether the archetype's table has a row for this standard.
    pub listed: bool,
    /// The row's "How it applies here" cell, links made absolute. For
    /// [`Applicability::NotApplicable`] this is the reason.
    pub note: Option<String>,
    /// The standard's own `## Checklist`. Empty when it doesn't apply.
    pub standard_items: Vec<ChecklistItem>,
    /// The archetype doc's own `## Checklist`. Empty when the standard doesn't apply, or
    /// when the standard is the archetype doc itself.
    pub archetype_items: Vec<ChecklistItem>,
}

/// A template in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplateSummary {
    pub path: String,
    pub placeholders: Vec<String>,
}

/// A skill in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    /// Asset paths relative to the skill directory, sorted.
    pub assets: Vec<String>,
}

/// Behaviour over one bundle. Cheap to share: wrap it in an `Arc`.
pub struct Engine {
    bundle: Arc<Bundle>,
    linker: Linker,
    index: Index,
    llms_index: String,
    llms_full: String,
}

impl Engine {
    /// Build the engine, its search index and the whole-corpus files.
    #[must_use]
    pub fn new(bundle: Arc<Bundle>) -> Self {
        Self {
            linker: bundle.linker(),
            index: Index::build(&bundle),
            llms_index: standards_core::llms_index(&bundle),
            llms_full: standards_core::llms_full(&bundle),
            bundle,
        }
    }

    /// The whole corpus as one markdown file (`/llms-full.txt`), composed once: the content
    /// cannot change while the process runs.
    #[must_use]
    pub fn llms_full(&self) -> &str {
        &self.llms_full
    }

    /// The corpus index per the llms.txt convention (`/llms.txt`), composed once.
    #[must_use]
    pub fn llms_index(&self) -> &str {
        &self.llms_index
    }

    #[must_use]
    pub fn bundle(&self) -> &Arc<Bundle> {
        &self.bundle
    }

    /// The commit the content came from.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.bundle.commit
    }

    /// The best `limit` hits for `query` over docs, sections, checklist items and skills.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        self.index.search(query, limit)
    }

    // ── docs ──

    #[must_use]
    pub fn docs(&self) -> Vec<DocSummary> {
        self.bundle
            .docs
            .iter()
            .map(|d| DocSummary {
                id: d.id.clone(),
                title: d.title.clone(),
                has_checklist: d.checklist.is_some(),
            })
            .collect()
    }

    /// A doc by id (`versioning`, `archetypes/cli-tool`), links absolute.
    pub fn doc(&self, id: &str) -> Result<DocView> {
        let doc = self.find_doc(id)?;
        Ok(DocView {
            id: doc.id.clone(),
            title: doc.title.clone(),
            markdown: self.linker.rewrite(&doc.id.path(), &doc.markdown),
            sections: doc
                .sections
                .iter()
                .map(|s| SectionInfo {
                    level: s.level,
                    title: s.title.clone(),
                    anchor: s.anchor.clone(),
                })
                .collect(),
        })
    }

    /// One section of a doc by anchor, links absolute. A leading `#` is accepted.
    pub fn doc_section(&self, id: &str, anchor: &str) -> Result<SectionView> {
        let doc = self.find_doc(id)?;
        let anchor = anchor.strip_prefix('#').unwrap_or(anchor);
        let section = doc.section(anchor).ok_or_else(|| Error::UnknownSection {
            doc: id.to_owned(),
            anchor: anchor.to_owned(),
        })?;
        Ok(SectionView {
            doc: doc.id.clone(),
            anchor: section.anchor.clone(),
            level: section.level,
            title: section.title.clone(),
            markdown: self
                .linker
                .rewrite(&doc.id.path(), &doc.markdown[section.range.clone()]),
        })
    }

    fn find_doc(&self, id: &str) -> Result<&Doc> {
        self.bundle
            .doc(id)
            .ok_or_else(|| Error::UnknownDoc { id: id.to_owned() })
    }

    // ── archetypes and checklists ──

    #[must_use]
    pub fn archetypes(&self) -> &[Archetype] {
        &self.bundle.archetypes
    }

    /// By name (`cli-tool`); `archetypes/cli-tool` is accepted too.
    pub fn archetype(&self, name: &str) -> Result<&Archetype> {
        let short = name.strip_prefix("archetypes/").unwrap_or(name);
        self.bundle
            .archetype(short)
            .ok_or_else(|| Error::UnknownArchetype {
                name: name.to_owned(),
            })
    }

    /// The checklist for `standard` (a doc id) in `archetype`: the standard's own items
    /// plus the archetype's, with how the archetype says the standard applies.
    ///
    /// A standard the archetype marks **N/A** is a value, not an error: `applicability`
    /// is [`Applicability::NotApplicable`], `note` carries the row's reason and both item
    /// lists are empty.
    pub fn checklist(&self, standard: &str, archetype: &str) -> Result<ComposedChecklist> {
        let doc = self.find_doc(standard)?;
        let arch = self.archetype(archetype)?;
        let row = arch.row(&doc.id);
        let applicability = row.map_or(Applicability::Applies, |r| r.applicability);
        let note = row.map(|r| self.linker.rewrite(&arch.doc.path(), &r.note));

        let (standard_items, archetype_items) = if applicability == Applicability::NotApplicable {
            (Vec::new(), Vec::new())
        } else {
            let arch_items = if arch.doc == doc.id {
                Vec::new()
            } else {
                self.bundle
                    .doc(arch.doc.as_str())
                    .map(|d| self.items(d))
                    .unwrap_or_default()
            };
            (self.items(doc), arch_items)
        };

        Ok(ComposedChecklist {
            standard: doc.id.clone(),
            archetype: arch.name.clone(),
            applicability,
            listed: row.is_some(),
            note,
            standard_items,
            archetype_items,
        })
    }

    /// A doc's checklist items with their text's links made absolute.
    fn items(&self, doc: &Doc) -> Vec<ChecklistItem> {
        let base = doc.id.path();
        doc.checklist
            .iter()
            .flat_map(|c| &c.items)
            .map(|item| ChecklistItem {
                text: self.linker.rewrite(&base, &item.text),
                links: item.links.clone(),
            })
            .collect()
    }

    // ── templates ──

    #[must_use]
    pub fn templates(&self) -> Vec<TemplateSummary> {
        self.bundle
            .templates
            .iter()
            .map(|t| TemplateSummary {
                path: t.path.clone(),
                placeholders: t.placeholders.iter().cloned().collect(),
            })
            .collect()
    }

    /// By path relative to `templates/` (`rust/build.rs`); a `templates/` prefix is
    /// accepted too.
    pub fn template(&self, path: &str) -> Result<&Template> {
        let unknown = || Error::UnknownTemplate {
            path: path.to_owned(),
        };
        let norm = normalize(path).ok_or_else(unknown)?;
        let rel = norm.strip_prefix("templates/").unwrap_or(&norm);
        self.bundle.template(rel).ok_or_else(unknown)
    }

    /// Render a template, replacing only the given `{{KEY}}`s (L8) and reporting the rest.
    pub fn render_template(&self, path: &str, vars: &BTreeMap<String, String>) -> Result<Rendered> {
        Ok(self.template(path)?.render(vars))
    }

    // ── skills ──

    #[must_use]
    pub fn skills(&self) -> Vec<SkillSummary> {
        self.bundle
            .skills
            .iter()
            .map(|s| SkillSummary {
                name: s.name.clone(),
                description: s.description.clone(),
                assets: s.assets.iter().map(|a| a.path.clone()).collect(),
            })
            .collect()
    }

    pub fn skill(&self, name: &str) -> Result<&Skill> {
        self.bundle.skill(name).ok_or_else(|| Error::UnknownSkill {
            name: name.to_owned(),
        })
    }

    /// An asset by its path relative to the skill directory, nesting kept
    /// (`scripts/obsq.sh`, L28). A path that escapes the skill directory is unknown.
    pub fn skill_asset(&self, name: &str, path: &str) -> Result<&Asset> {
        let skill = self.skill(name)?;
        normalize(path)
            .and_then(|p| skill.asset(&p))
            .ok_or_else(|| Error::UnknownAsset {
                skill: name.to_owned(),
                path: path.to_owned(),
            })
    }

    // ── manifest ──

    /// The manifest for a server at `server_version` (its derived version).
    #[must_use]
    pub fn manifest(&self, server_version: &str) -> Manifest {
        self.bundle.manifest(server_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCH: &str = "# Archetype: Lib\n\n| Standard | How it applies here |\n|---|---|\n\
                        | [V](../v.md) | **N/A.** See [w](../w.md). |\n\
                        | [W](../w.md) | **Adapted** — lighter. |\n\
                        | [Lib](lib.md) | Self. |\n\n\
                        ## Checklist\n\n- [ ] Arch item, see [v](../v.md).\n";

    fn engine() -> Engine {
        let files: Vec<(&str, &str)> = vec![
            (
                "docs/v.md",
                "# V\n\n## Rule\n\nx\n\n## Checklist\n\n- [ ] V item.\n",
            ),
            (
                "docs/w.md",
                "# W\n\n## Part\n\nSee [v](v.md#rule).\n\n### Sub\n\ns\n\n## Checklist\n\n- [ ] W item, [v](v.md).\n",
            ),
            ("docs/u.md", "# U\n\nNo checklist.\n"),
            ("docs/archetypes/lib.md", ARCH),
        ];
        let b = Bundle::from_files(
            files.into_iter().map(|(p, s)| (p, s.as_bytes().to_vec())),
            "abc",
        )
        .unwrap();
        Engine::new(Arc::new(b))
    }

    #[test]
    fn composes_checklists() {
        let e = engine();
        let na = e.checklist("v", "lib").unwrap();
        assert_eq!(na.applicability, Applicability::NotApplicable);
        assert!(na.listed);
        assert_eq!(
            na.note.as_deref(),
            Some("**N/A.** See [w](https://standards.thmsn.dev/d/w).")
        );
        assert_eq!((na.standard_items.len(), na.archetype_items.len()), (0, 0));

        let w = e.checklist("w", "archetypes/lib").unwrap();
        assert_eq!(w.applicability, Applicability::Adapted);
        assert_eq!(w.archetype, "lib");
        assert_eq!(
            w.standard_items[0].text,
            "W item, [v](https://standards.thmsn.dev/d/v)."
        );
        assert_eq!(
            w.archetype_items[0].text,
            "Arch item, see [v](https://standards.thmsn.dev/d/v)."
        );

        let unlisted = e.checklist("u", "lib").unwrap();
        assert_eq!(unlisted.applicability, Applicability::Applies);
        assert!(!unlisted.listed && unlisted.note.is_none());
        assert_eq!(unlisted.standard_items.len(), 0);
        assert_eq!(unlisted.archetype_items.len(), 1);

        let own = e.checklist("archetypes/lib", "lib").unwrap();
        assert_eq!(own.standard_items.len(), 1);
        assert_eq!(own.archetype_items.len(), 0);

        assert_eq!(
            e.checklist("nope", "lib").unwrap_err(),
            Error::UnknownDoc { id: "nope".into() }
        );
        assert_eq!(
            e.checklist("v", "nope").unwrap_err(),
            Error::UnknownArchetype {
                name: "nope".into()
            }
        );
    }

    #[test]
    fn docs_and_sections() {
        let e = engine();
        assert_eq!(e.commit(), "abc");
        assert_eq!(e.bundle().docs.len(), 4);
        let list = e.docs();
        assert!(
            list.iter()
                .any(|d| d.id.as_str() == "u" && !d.has_checklist)
        );
        let w = e.doc("w").unwrap();
        assert!(
            w.markdown
                .contains("(https://standards.thmsn.dev/d/v#rule)")
        );
        assert_eq!(
            w.sections
                .iter()
                .map(|s| s.anchor.as_str())
                .collect::<Vec<_>>(),
            ["part", "sub", "checklist"]
        );
        let part = e.doc_section("w", "#part").unwrap();
        assert_eq!(part.level, 2);
        assert_eq!(part.title, "Part");
        assert!(part.markdown.starts_with("## Part"));
        assert!(part.markdown.contains("### Sub"));
        assert!(
            part.markdown
                .contains("https://standards.thmsn.dev/d/v#rule")
        );
        assert_eq!(
            e.doc_section("w", "nope").unwrap_err().to_string(),
            "doc `w` has no section `nope`"
        );
        assert_eq!(
            e.doc_section("nope", "x").unwrap_err().to_string(),
            "no doc `nope`"
        );
        assert_eq!(e.archetypes().len(), 1);
    }

    #[test]
    fn errors_serialize_with_a_kind() {
        let v = serde_json::to_value(Error::UnknownAsset {
            skill: "s".into(),
            path: "p".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "unknown_asset");
        assert_eq!(
            Error::UnknownTemplate { path: "x".into() }.to_string(),
            "no template `x`"
        );
        assert_eq!(
            Error::UnknownSkill { name: "x".into() }.to_string(),
            "no skill `x`"
        );
    }
}
