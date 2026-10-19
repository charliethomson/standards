//! Skills: `skills/<name>/SKILL.md` plus every other file in that directory as an asset.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// A file bundled with a skill (`assets/review.template.md`, `scripts/obsq.sh`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Asset {
    /// Relative to the skill's directory.
    pub path: String,
    #[serde(skip_serializing)]
    pub bytes: Vec<u8>,
    /// Hex sha256 of `bytes`.
    pub sha256: String,
}

impl Asset {
    #[must_use]
    pub fn new(path: String, bytes: Vec<u8>) -> Self {
        let sha256 = sha256_hex(&bytes);
        Self {
            path,
            bytes,
            sha256,
        }
    }
}

/// A parsed skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// `SKILL.md` after the frontmatter.
    pub body: String,
    /// Sorted by path.
    pub assets: Vec<Asset>,
    /// Hex sha256 of `description`'s UTF-8 bytes.
    pub description_sha256: String,
    /// Hex sha256 over the body and the assets: the body's bytes, then for each asset in
    /// path order `\0`, its path, `\0`, its hex sha256. Changes when the body or any
    /// asset's path or bytes change.
    pub content_sha256: String,
}

impl Skill {
    /// Parse `SKILL.md` for the skill in directory `dir`. Its frontmatter must carry `name`
    /// (equal to `dir`) and `description`.
    pub fn parse(dir: &str, skill_md: &str, mut assets: Vec<Asset>) -> Result<Self> {
        let path = format!("skills/{dir}/SKILL.md");
        let err = |reason: &str| Error::Frontmatter {
            path: path.clone(),
            reason: reason.to_owned(),
        };
        let (front, body) =
            split_frontmatter(skill_md).ok_or_else(|| err("missing `---` block"))?;
        let fields = parse_frontmatter(front);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };
        let name = get("name")
            .filter(|n| !n.is_empty())
            .ok_or_else(|| err("no `name`"))?;
        if name != dir {
            return Err(err(&format!(
                "name `{name}` does not match directory `{dir}`"
            )));
        }
        let description = get("description")
            .filter(|d| !d.is_empty())
            .ok_or_else(|| err("no `description`"))?;

        assets.sort_by(|a, b| a.path.cmp(&b.path));
        let mut content = Sha256::new();
        content.update(body.as_bytes());
        for a in &assets {
            content.update(b"\0");
            content.update(a.path.as_bytes());
            content.update(b"\0");
            content.update(a.sha256.as_bytes());
        }

        Ok(Self {
            description_sha256: sha256_hex(description.as_bytes()),
            content_sha256: hex::encode(content.finalize()),
            name,
            description,
            body: body.to_owned(),
            assets,
        })
    }

    /// The asset at `path` (relative to the skill directory).
    #[must_use]
    pub fn asset(&self, path: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.path == path)
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `(frontmatter, body)` when the text opens with a `---` line closed by another.
fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..at], &rest[at + line.len()..]));
        }
        at += line.len();
    }
    None
}

/// The YAML subset skill frontmatter uses: `key: value` with optional quotes, plain
/// values continued on indented lines, and `>` / `|` block scalars.
fn parse_frontmatter(front: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String, char)> = Vec::new();
    for line in front.lines() {
        let indented = line.starts_with([' ', '\t']);
        if indented || line.trim().is_empty() {
            if let Some((_, value, style)) = out.last_mut() {
                let piece = line.trim();
                match style {
                    '|' => {
                        value.push_str(piece);
                        value.push('\n');
                    }
                    _ if piece.is_empty() => {}
                    _ => {
                        if !value.is_empty() {
                            value.push(' ');
                        }
                        value.push_str(piece);
                    }
                }
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let (value, style) = match value.chars().next() {
            Some(c @ ('>' | '|')) => (String::new(), c),
            _ => (unquote(value).to_owned(), ' '),
        };
        out.push((key.trim().to_owned(), value, style));
    }
    out.into_iter()
        .map(|(k, v, _)| (k, v.trim_end().to_owned()))
        .collect()
}

fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_hashes() {
        let md = "---\nname: thmsn-x\ndescription: Does x. Use when y.\n---\n\n# X\n\nbody\n";
        let s = Skill::parse(
            "thmsn-x",
            md,
            vec![
                Asset::new("b.md".into(), b"b".to_vec()),
                Asset::new("a.md".into(), b"a".to_vec()),
            ],
        )
        .unwrap();
        assert_eq!(s.name, "thmsn-x");
        assert_eq!(s.description, "Does x. Use when y.");
        assert_eq!(s.body, "\n# X\n\nbody\n");
        assert_eq!(
            s.assets.iter().map(|a| a.path.as_str()).collect::<Vec<_>>(),
            ["a.md", "b.md"]
        );
        assert_eq!(s.description_sha256, sha256_hex(b"Does x. Use when y."));
        assert_eq!(s.asset("a.md").unwrap().bytes, b"a");
        assert!(s.asset("c.md").is_none());

        let changed_asset = Skill::parse(
            "thmsn-x",
            md,
            vec![Asset::new("a.md".into(), b"A".to_vec())],
        )
        .unwrap();
        let no_assets = Skill::parse("thmsn-x", md, vec![]).unwrap();
        assert_ne!(s.content_sha256, changed_asset.content_sha256);
        assert_ne!(changed_asset.content_sha256, no_assets.content_sha256);
        assert_eq!(no_assets.content_sha256, sha256_hex(b"\n# X\n\nbody\n"));
        assert_eq!(s.description_sha256, no_assets.description_sha256);
    }

    #[test]
    fn yaml_subset() {
        let front = "name: 'q'\ndescription: \"first\n  second\"\nfolded: >\n  a\n  b\n\nliteral: |\n  l1\n  l2\nnot a field\n";
        let f = parse_frontmatter(front);
        let get = |k: &str| f.iter().find(|(key, _)| key == k).unwrap().1.clone();
        assert_eq!(get("name"), "q");
        assert_eq!(get("description"), "\"first second\"");
        assert_eq!(get("folded"), "a b");
        assert_eq!(get("literal"), "l1\nl2");
        assert_eq!(parse_frontmatter("  orphan continuation\n"), []);
    }

    #[test]
    fn frontmatter_errors() {
        let e = |md: &str| Skill::parse("s", md, vec![]).unwrap_err().to_string();
        assert_eq!(
            e("# no frontmatter"),
            "skills/s/SKILL.md: frontmatter: missing `---` block"
        );
        assert_eq!(
            e("---\nname: s\n"),
            "skills/s/SKILL.md: frontmatter: missing `---` block"
        );
        assert_eq!(
            e("---\ndescription: d\n---\n"),
            "skills/s/SKILL.md: frontmatter: no `name`"
        );
        assert_eq!(
            e("---\nname: s\n---\n"),
            "skills/s/SKILL.md: frontmatter: no `description`"
        );
        assert_eq!(
            e("---\nname: t\ndescription: d\n---\n"),
            "skills/s/SKILL.md: frontmatter: name `t` does not match directory `s`"
        );
        assert!(
            Skill::parse(
                "s",
                "---\r\nname: s\r\ndescription: d\r\n---\r\nbody",
                vec![]
            )
            .is_ok()
        );
    }
}
