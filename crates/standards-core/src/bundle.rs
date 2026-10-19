//! The whole served content of one commit.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::archetype::Archetype;
use crate::doc::{Doc, DocId};
use crate::error::{Error, Result};
use crate::links::Linker;
use crate::manifest::{MANIFEST_SCHEMA, Manifest, SkillEntry};
use crate::path::{is_served, normalize};
use crate::skill::{Asset, Skill};
use crate::template::Template;
use crate::{CLI_INSTALL_URL, MIN_CLI_VERSION};

/// Docs, archetypes, skills and templates parsed from one commit's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bundle {
    pub commit: String,
    /// Every `docs/**/*.md`, archetypes included, sorted by id.
    pub docs: Vec<Doc>,
    /// `docs/archetypes/*.md`, sorted by name.
    pub archetypes: Vec<Archetype>,
    /// `skills/<name>/`, sorted by name.
    pub skills: Vec<Skill>,
    /// `templates/**`, sorted by path.
    pub templates: Vec<Template>,
    pub changelog: Option<String>,
}

impl Bundle {
    /// Build the bundle from `(repo-relative path, bytes)` pairs. Paths outside the served
    /// allowlist ([`is_served`]) are skipped, so a caller may pass the whole tree.
    pub fn from_files<P, I>(files: I, commit: &str) -> Result<Self>
    where
        P: AsRef<str>,
        I: IntoIterator<Item = (P, Vec<u8>)>,
    {
        let mut served: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (path, bytes) in files {
            let path = path.as_ref();
            let norm = normalize(path).ok_or_else(|| Error::InvalidPath {
                path: path.to_owned(),
            })?;
            if !is_served(&norm) {
                continue;
            }
            if served.insert(norm.clone(), bytes).is_some() {
                return Err(Error::Duplicate { path: norm });
            }
        }

        let mut docs = Vec::new();
        let mut templates = Vec::new();
        let mut changelog = None;
        let mut skill_files: BTreeMap<String, (Option<String>, Vec<Asset>)> = BTreeMap::new();
        for (path, bytes) in served {
            if path == "CHANGELOG.md" {
                changelog = Some(utf8(&path, bytes)?);
            } else if let Some(id) = DocId::from_path(&path) {
                docs.push(Doc::parse(id, utf8(&path, bytes)?)?);
            } else if let Some(rest) = path.strip_prefix("templates/") {
                templates.push(Template::new(rest.to_owned(), utf8(&path, bytes)?));
            } else if let Some((name, file)) =
                path.strip_prefix("skills/").and_then(|r| r.split_once('/'))
            {
                let entry = skill_files.entry(name.to_owned()).or_default();
                if file == "SKILL.md" {
                    entry.0 = Some(utf8(&path, bytes)?);
                } else {
                    entry.1.push(Asset::new(file.to_owned(), bytes));
                }
            }
        }

        let skills = skill_files
            .into_iter()
            .map(|(name, (md, assets))| {
                let md = md.ok_or_else(|| Error::Frontmatter {
                    path: format!("skills/{name}/SKILL.md"),
                    reason: "file missing".into(),
                })?;
                Skill::parse(&name, &md, assets)
            })
            .collect::<Result<Vec<_>>>()?;
        let archetypes = docs
            .iter()
            .filter(|d| d.id.as_str().starts_with("archetypes/"))
            .map(Archetype::from_doc)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            commit: commit.to_owned(),
            docs,
            archetypes,
            skills,
            templates,
            changelog,
        })
    }

    #[must_use]
    pub fn doc(&self, id: &str) -> Option<&Doc> {
        self.docs.iter().find(|d| d.id.as_str() == id)
    }

    /// By name (`cli-tool`).
    #[must_use]
    pub fn archetype(&self, name: &str) -> Option<&Archetype> {
        self.archetypes.iter().find(|a| a.name == name)
    }

    #[must_use]
    pub fn skill(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// By path relative to `templates/`.
    #[must_use]
    pub fn template(&self, path: &str) -> Option<&Template> {
        self.templates.iter().find(|t| t.path == path)
    }

    /// A [`Linker`] that knows this bundle's docs and templates.
    #[must_use]
    pub fn linker(&self) -> Linker {
        Linker {
            commit: self.commit.clone(),
            docs: self.docs.iter().map(|d| d.id.clone()).collect(),
            templates: self.templates.iter().map(|t| t.path.clone()).collect(),
        }
    }

    /// A doc's markdown with its links made absolute (L25), for HTML and MCP/REST alike.
    #[must_use]
    pub fn doc_markdown(&self, id: &DocId) -> Option<String> {
        let doc = self.doc(id.as_str())?;
        Some(self.linker().rewrite(&id.path(), &doc.markdown))
    }

    /// The manifest this bundle publishes, for a server at `server_version`.
    #[must_use]
    pub fn manifest(&self, server_version: &str) -> Manifest {
        Manifest {
            schema: MANIFEST_SCHEMA,
            commit: self.commit.clone(),
            server_version: server_version.to_owned(),
            min_cli_version: MIN_CLI_VERSION.to_owned(),
            cli_install_url: CLI_INSTALL_URL.to_owned(),
            archetypes: self.archetypes.iter().map(|a| a.name.clone()).collect(),
            skills: self
                .skills
                .iter()
                .map(|s| SkillEntry {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    description_sha256: s.description_sha256.clone(),
                    content_sha256: s.content_sha256.clone(),
                    assets: s.assets.iter().map(|a| a.path.clone()).collect(),
                })
                .collect(),
        }
    }
}

fn utf8(path: &str, bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| Error::NotUtf8 {
        path: path.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        list.iter()
            .map(|(p, b)| ((*p).to_owned(), b.as_bytes().to_vec()))
            .collect()
    }

    const ARCH: &str = "# Archetype: Lib\n\n| Standard | How it applies here |\n|---|---|\n| [V](../v.md) | **N/A.** |\n";

    #[test]
    fn classifies_and_skips_unserved() {
        let b = Bundle::from_files(
            files(&[
                (
                    "docs/v.md",
                    "# V\n\nSee [lib](archetypes/lib.md).\n\n## Checklist\n\n- [ ] x\n",
                ),
                ("./docs/archetypes/lib.md", ARCH),
                ("skills/README.md", "index"),
                (
                    "skills/s/SKILL.md",
                    "---\nname: s\ndescription: d\n---\nbody",
                ),
                ("skills/s/assets/a.md", "{{PRODUCT}}"),
                ("templates/x.md", "{{PRODUCT}}"),
                ("CHANGELOG.md", "# Changelog"),
                ("bin/standards", "#!/bin/sh"),
                (".mcp.json", "{}"),
                ("docs/img.png", "png"),
            ]),
            "abc1234",
        )
        .unwrap();
        assert_eq!(b.commit, "abc1234");
        assert_eq!(
            b.docs.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["archetypes/lib", "v"]
        );
        assert_eq!(b.archetype("lib").unwrap().rows.len(), 1);
        assert!(b.archetype("nope").is_none());
        assert_eq!(b.skill("s").unwrap().assets.len(), 1);
        assert!(b.skill("README.md").is_none());
        assert_eq!(b.template("x.md").unwrap().placeholders.len(), 1);
        assert_eq!(b.changelog.as_deref(), Some("# Changelog"));
        assert_eq!(
            b.doc_markdown(&DocId::new("v")).unwrap(),
            "# V\n\nSee [lib](https://standards.thmsn.dev/d/archetypes/lib).\n\n## Checklist\n\n- [ ] x\n"
        );
        assert!(b.doc_markdown(&DocId::new("nope")).is_none());

        let m = b.manifest("0.1.5");
        assert_eq!(m.schema, MANIFEST_SCHEMA);
        assert_eq!(m.server_version, "0.1.5");
        assert_eq!(m.min_cli_version, MIN_CLI_VERSION);
        assert_eq!(m.cli_install_url, CLI_INSTALL_URL);
        assert_eq!(m.archetypes, ["lib"]);
        assert_eq!(m.skills[0].assets, ["assets/a.md"]);
        assert_eq!(m.skills[0].content_sha256, b.skills[0].content_sha256);
    }

    #[test]
    fn errors() {
        let e = |list: &[(&str, &str)]| Bundle::from_files(files(list), "c").unwrap_err();
        assert_eq!(
            e(&[("../x", "")]),
            Error::InvalidPath {
                path: "../x".into()
            }
        );
        assert_eq!(
            e(&[("docs/v.md", "# V"), ("docs//v.md", "# V")]),
            Error::Duplicate {
                path: "docs/v.md".into()
            }
        );
        assert_eq!(
            e(&[("docs/v.md", "no title")]),
            Error::MissingTitle {
                path: "docs/v.md".into()
            }
        );
        assert_eq!(
            e(&[("skills/s/assets/a", "x")]),
            Error::Frontmatter {
                path: "skills/s/SKILL.md".into(),
                reason: "file missing".into()
            }
        );
        assert_eq!(
            e(&[("docs/archetypes/x.md", "# X\n")]),
            Error::MissingStandardsTable {
                path: "docs/archetypes/x.md".into()
            }
        );
        let bad = Bundle::from_files(vec![("templates/x", vec![0xff, 0xfe])], "c").unwrap_err();
        assert_eq!(
            bad,
            Error::NotUtf8 {
                path: "templates/x".into()
            }
        );
    }
}
