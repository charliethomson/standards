use serde::Serialize;

/// Everything that can be wrong with the content handed to [`crate::Bundle::from_files`]
/// or [`crate::StandardsConf::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    #[error("{path}: not a valid repo-relative path")]
    InvalidPath { path: String },

    #[error("{path}: not UTF-8")]
    NotUtf8 { path: String },

    #[error("{path}: no `# ` title")]
    MissingTitle { path: String },

    #[error("{path}: listed twice")]
    Duplicate { path: String },

    #[error("{path}: frontmatter: {reason}")]
    Frontmatter { path: String, reason: String },

    #[error("{path}: no `| Standard | How it applies here |` table")]
    MissingStandardsTable { path: String },

    #[error(".standards.conf line {line}: {reason}")]
    Conf { line: usize, reason: String },

    #[error(".standards.conf: {key} missing")]
    ConfMissing { key: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
