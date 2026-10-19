//! `/manifest.json`: what the cli syncs against (L9).

use serde::{Deserialize, Serialize};

/// Bumped when the manifest's shape changes incompatibly.
pub const MANIFEST_SCHEMA: u32 = 1;

/// The server's description of the content it serves and the cli it expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    /// The commit the content was embedded from.
    pub commit: String,
    pub server_version: String,
    /// The oldest cli this server supports ([`crate::MIN_CLI_VERSION`]).
    pub min_cli_version: String,
    pub cli_install_url: String,
    /// Archetype names (`cli-tool`, ...), sorted.
    pub archetypes: Vec<String>,
    /// Sorted by name.
    pub skills: Vec<SkillEntry>,
}

/// One skill as the cli's stub writer needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub description_sha256: String,
    pub content_sha256: String,
    /// Asset paths relative to the skill directory, sorted.
    pub assets: Vec<String>,
}
