use serde::Serialize;

/// A lookup the bundle can't answer. A standard that doesn't apply to an archetype is
/// **not** an error: it is [`standards_core::Applicability::NotApplicable`] on the
/// composed checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    #[error("no doc `{id}`")]
    UnknownDoc { id: String },

    #[error("doc `{doc}` has no section `{anchor}`")]
    UnknownSection { doc: String, anchor: String },

    #[error("no archetype `{name}`")]
    UnknownArchetype { name: String },

    #[error("no skill `{name}`")]
    UnknownSkill { name: String },

    #[error("skill `{skill}` has no asset `{path}`")]
    UnknownAsset { skill: String, path: String },

    #[error("no template `{path}`")]
    UnknownTemplate { path: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
