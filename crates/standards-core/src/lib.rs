//! The standards repo's content, parsed from bytes.
//!
//! `standards-core` is the model both the server (`standards-api`) and the `standards` cli
//! share. It performs **no I/O** (docs/service-architecture.md): callers hand it
//! `(relative path, bytes)` pairs and a commit, and get back a [`Bundle`] of docs,
//! archetypes, skills and templates, plus the [`Manifest`] the cli syncs against.

mod archetype;
mod bundle;
mod checklist;
mod conf;
mod doc;
mod error;
mod links;
mod llms;
mod manifest;
mod path;
mod skill;
mod template;
mod version;

pub use archetype::{Applicability, Archetype, ArchetypeRow, StandardRef};
pub use bundle::Bundle;
pub use checklist::{Checklist, ChecklistItem};
pub use conf::StandardsConf;
pub use doc::{Doc, DocId, Section, slugify};
pub use error::{Error, Result};
pub use links::{Link, Linker, Target, resolve_link};
pub use llms::{ARCHETYPES_GROUP, OutlineEntry, OutlineGroup, llms_full, llms_index, outline};
pub use manifest::{MANIFEST_SCHEMA, Manifest, SkillEntry};
pub use path::{is_served, normalize};
pub use skill::{Asset, Skill};
pub use template::{Rendered, Template, placeholders, render};
pub use version::DerivedVersion;

/// Public origin of the hosted server. Rewritten doc links point here.
pub const ORIGIN: &str = "https://standards.thmsn.dev";

/// Where the cli is installed from: a 302 to the registry's install script.
pub const CLI_INSTALL_URL: &str = "https://standards.thmsn.dev/install";

/// GitHub repository the content comes from; links to unserved repo files point here.
pub const REPOSITORY: &str = "https://github.com/charliethomson/standards";

/// Product identifier of the server binary (docs/identifiers.md).
pub const SERVER_PRODUCT: &str = "dev.thmsn.standards.server";

/// Product identifier of the cli binary (docs/identifiers.md).
pub const CLI_PRODUCT: &str = "dev.thmsn.standards.cli";

/// Environment variable prefix for both binaries' configuration.
pub const ENV_PREFIX: &str = "STANDARDS_";

/// The oldest cli the current server contract supports. **Bumped by hand** whenever the
/// manifest or REST shape changes in a way an older cli would misread; `standards doctor`
/// fails `cli.current` below it. `0.0.0` until the first cli ships.
pub const MIN_CLI_VERSION: &str = "0.0.0";

/// This build's derived version (`MAJOR.MINOR.<commit count>`, docs/versioning.md).
pub const VERSION: &str = env!("APP_VERSION");

/// Full build metadata embedded at compile time, when the build had a repository to read.
#[must_use]
pub fn build_info() -> Option<libbuildinfo::BuildInfo> {
    libbuildinfo::load_build_info!(optional).and_then(std::result::Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derived version and the embedded build info describe the same commit.
    #[test]
    fn version_and_build_info_agree() {
        let version: DerivedVersion = VERSION.parse().unwrap();
        if let Some(count) = build_info().and_then(|bi| bi.git.commit_count) {
            assert_eq!(version.count, count);
        }
    }
}
