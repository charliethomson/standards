//! The REST mirror's wire types (poem-openapi `Object`s), converted from the engine's.
//!
//! Field names match the engine and core types' serde output (snake case), so a client that
//! deserializes into `standards-core` types and one generated from `api/openapi.json` read
//! the same JSON.

use poem_openapi::{Enum, Object};
use standards_core::Applicability as CoreApplicability;
use standards_engine::{
    ComposedChecklist, DocSummary as EngineDocSummary, DocView, Hit, HitKind as EngineHitKind,
    SkillSummary as EngineSkillSummary, TemplateSummary as EngineTemplateSummary,
};

/// The error body every client reads.
#[derive(Debug, Clone, Object)]
pub struct ErrorBody {
    /// For a person.
    pub message: String,
    /// Stable error key (`dev.thmsn.standards.server.content.error.unknown_doc`).
    #[oai(rename = "$type")]
    pub kind: String,
    /// The request's trace id, when traces are exported.
    #[oai(rename = "traceId")]
    pub trace_id: Option<String>,
}

/// A doc in a listing.
#[derive(Debug, Clone, Object)]
pub struct DocSummary {
    /// Path under `docs/` without `.md` (`versioning`, `archetypes/cli-tool`).
    pub id: String,
    pub title: String,
    pub has_checklist: bool,
}

impl From<EngineDocSummary> for DocSummary {
    fn from(d: EngineDocSummary) -> Self {
        Self {
            id: d.id.to_string(),
            title: d.title,
            has_checklist: d.has_checklist,
        }
    }
}

/// A heading in a doc's outline.
#[derive(Debug, Clone, Object)]
pub struct Section {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

/// A whole doc, its links absolute.
#[derive(Debug, Clone, Object)]
pub struct Doc {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub sections: Vec<Section>,
}

impl From<DocView> for Doc {
    fn from(d: DocView) -> Self {
        Self {
            id: d.id.to_string(),
            title: d.title,
            markdown: d.markdown,
            sections: d
                .sections
                .into_iter()
                .map(|s| Section {
                    level: s.level,
                    title: s.title,
                    anchor: s.anchor,
                })
                .collect(),
        }
    }
}

/// How a standard applies to an archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[oai(rename_all = "snake_case")]
pub enum Applicability {
    Applies,
    Defining,
    Adapted,
    Optional,
    NotApplicable,
}

impl From<CoreApplicability> for Applicability {
    fn from(a: CoreApplicability) -> Self {
        match a {
            CoreApplicability::Applies => Self::Applies,
            CoreApplicability::Defining => Self::Defining,
            CoreApplicability::Adapted => Self::Adapted,
            CoreApplicability::Optional => Self::Optional,
            CoreApplicability::NotApplicable => Self::NotApplicable,
        }
    }
}

/// One checklist item: markdown on one line, links absolute.
#[derive(Debug, Clone, Object)]
pub struct ChecklistItem {
    pub text: String,
}

/// The checklist for one standard in one archetype.
#[derive(Debug, Clone, Object)]
pub struct Checklist {
    /// The standard's doc id.
    pub standard: String,
    /// The archetype's name (`cli-tool`).
    pub archetype: String,
    pub applicability: Applicability,
    /// Whether the archetype's table has a row for this standard.
    pub listed: bool,
    /// The row's "How it applies here" cell; for `not_applicable`, the reason.
    pub note: Option<String>,
    /// The standard's own checklist. Empty when it does not apply.
    pub standard_items: Vec<ChecklistItem>,
    /// The archetype doc's checklist. Empty when the standard does not apply.
    pub archetype_items: Vec<ChecklistItem>,
}

impl From<ComposedChecklist> for Checklist {
    fn from(c: ComposedChecklist) -> Self {
        let items = |v: Vec<standards_core::ChecklistItem>| {
            v.into_iter()
                .map(|i| ChecklistItem { text: i.text })
                .collect()
        };
        Self {
            standard: c.standard.to_string(),
            archetype: c.archetype,
            applicability: c.applicability.into(),
            listed: c.listed,
            note: c.note,
            standard_items: items(c.standard_items),
            archetype_items: items(c.archetype_items),
        }
    }
}

/// A template in a listing.
#[derive(Debug, Clone, Object)]
pub struct TemplateSummary {
    /// Relative to `templates/` (`rust/build.rs`).
    pub path: String,
    /// Every `{{KEY}}` in the template.
    pub placeholders: Vec<String>,
}

impl From<EngineTemplateSummary> for TemplateSummary {
    fn from(t: EngineTemplateSummary) -> Self {
        Self {
            path: t.path,
            placeholders: t.placeholders,
        }
    }
}

/// A rendered template.
#[derive(Debug, Clone, Object)]
pub struct RenderedTemplate {
    pub path: String,
    pub body: String,
    /// Placeholders left in `body` because no value was given, sorted.
    pub unfilled: Vec<String>,
}

/// A skill in a listing.
#[derive(Debug, Clone, Object)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    /// Asset paths relative to the skill directory, sorted.
    pub assets: Vec<String>,
}

impl From<EngineSkillSummary> for SkillSummary {
    fn from(s: EngineSkillSummary) -> Self {
        Self {
            name: s.name,
            description: s.description,
            assets: s.assets,
        }
    }
}

/// A file bundled with a skill.
#[derive(Debug, Clone, Object)]
pub struct AssetInfo {
    /// Relative to the skill directory (`scripts/obsq.sh`).
    pub path: String,
    pub sha256: String,
}

/// A whole skill.
#[derive(Debug, Clone, Object)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// `SKILL.md` after the frontmatter, as written.
    pub body: String,
    pub assets: Vec<AssetInfo>,
    pub description_sha256: String,
    pub content_sha256: String,
}

impl From<&standards_core::Skill> for Skill {
    fn from(s: &standards_core::Skill) -> Self {
        Self {
            name: s.name.clone(),
            description: s.description.clone(),
            body: s.body.clone(),
            assets: s
                .assets
                .iter()
                .map(|a| AssetInfo {
                    path: a.path.clone(),
                    sha256: a.sha256.clone(),
                })
                .collect(),
            description_sha256: s.description_sha256.clone(),
            content_sha256: s.content_sha256.clone(),
        }
    }
}

/// What a search hit points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[oai(rename_all = "snake_case")]
pub enum HitKind {
    Doc,
    Skill,
}

/// One search result.
#[derive(Debug, Clone, Object)]
pub struct SearchHit {
    pub kind: HitKind,
    /// The doc id, or the skill name for a skill hit.
    pub doc: String,
    /// The section anchor; absent for a whole doc or a skill.
    pub anchor: Option<String>,
    pub title: String,
    pub snippet: String,
    /// Higher is better; comparable within one query only.
    pub score: f32,
}

impl From<Hit> for SearchHit {
    fn from(h: Hit) -> Self {
        Self {
            kind: match h.kind {
                EngineHitKind::Doc => HitKind::Doc,
                EngineHitKind::Skill => HitKind::Skill,
            },
            doc: h.doc,
            anchor: h.anchor,
            title: h.title,
            snippet: h.snippet,
            score: h.score,
        }
    }
}

/// One skill as the cli's stub writer needs it.
#[derive(Debug, Clone, Object)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub description_sha256: String,
    pub content_sha256: String,
    pub assets: Vec<String>,
}

/// What the cli syncs against; also served at `/manifest.json`.
#[derive(Debug, Clone, Object)]
pub struct Manifest {
    pub schema: u32,
    /// The commit the content was embedded from.
    pub commit: String,
    pub server_version: String,
    /// The oldest cli this server supports.
    pub min_cli_version: String,
    pub cli_install_url: String,
    /// Archetype names, sorted.
    pub archetypes: Vec<String>,
    /// Sorted by name.
    pub skills: Vec<SkillEntry>,
}

impl From<standards_core::Manifest> for Manifest {
    fn from(m: standards_core::Manifest) -> Self {
        Self {
            schema: m.schema,
            commit: m.commit,
            server_version: m.server_version,
            min_cli_version: m.min_cli_version,
            cli_install_url: m.cli_install_url,
            archetypes: m.archetypes,
            skills: m
                .skills
                .into_iter()
                .map(|s| SkillEntry {
                    name: s.name,
                    description: s.description,
                    description_sha256: s.description_sha256,
                    content_sha256: s.content_sha256,
                    assets: s.assets,
                })
                .collect(),
        }
    }
}
