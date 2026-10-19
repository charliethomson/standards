//! The files `init` and `new` write, rendered by the server (L8) and never clobbering.
//!
//! Both commands render everything first and write after, so a server that is down or a
//! template that is missing leaves the directory untouched. A file that already exists is
//! skipped, never overwritten, including `.standards.conf`.
//!
//! ## What gets written
//!
//! Every archetype ([`base_files`], the port of `bin/standards install` minus the skill
//! symlinks and `.mcp.json`):
//!
//! | File | From |
//! |---|---|
//! | `.standards.conf` | `StandardsConf::to_conf_string` |
//! | `AGENTS.md` | `consuming-repo-AGENTS.md` (`<!-- -->` blocks and leading blank lines dropped, as `bin/standards` did) |
//! | `SHORTCUTS.md` | `SHORTCUTS.md` |
//! | `README.md` | `README.library.md` for `library` and `cli-tool`, else `README.product.md` |
//!
//! `new` adds a deliberately small per-archetype set ([`extra_files`]): only templates
//! whose destination the archetype's "Repo shape" names and that need nothing beyond the
//! identity to fill. The rest (server crates, deploy, workflows) is the
//! `thmsn-standards-init` skill's job, since it needs choices a scaffold can't make.
//!
//! | Archetype | Extra files |
//! |---|---|
//! | `full-stack-product` | `VERSIONING.md`, `branding/build.py` (executable) |
//! | `library` | `VERSIONING.md` |
//! | `cli-tool` | `VERSIONING.md`, `build.rs` (from `rust/build.rs`) |
//! | `orchestration`, `vendored-app`, anything else | none |

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use standards_core::{Manifest, StandardsConf};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::output::{Outcome, Status};

/// Where the identity lives in a repo.
pub const CONF_FILE: &str = ".standards.conf";

/// The archetype `init` assumes when none is given or recorded (as `bin/standards` did).
pub const DEFAULT_ARCHETYPE: &str = "full-stack-product";

/// A file written from a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSpec {
    /// Relative to the repo root.
    pub dest: &'static str,
    /// Relative to `templates/`.
    pub template: &'static str,
    pub executable: bool,
}

const fn spec(dest: &'static str, template: &'static str) -> FileSpec {
    FileSpec {
        dest,
        template,
        executable: false,
    }
}

/// The template-backed files every repo gets (after `.standards.conf`).
#[must_use]
pub fn base_files(archetype: &str) -> Vec<FileSpec> {
    let readme = match archetype {
        "library" | "cli-tool" => "README.library.md",
        _ => "README.product.md",
    };
    vec![
        spec("AGENTS.md", "consuming-repo-AGENTS.md"),
        spec("SHORTCUTS.md", "SHORTCUTS.md"),
        spec("README.md", readme),
    ]
}

/// What `new` adds for `archetype`. See the module docs for the reasoning.
#[must_use]
pub fn extra_files(archetype: &str) -> Vec<FileSpec> {
    match archetype {
        "full-stack-product" => vec![
            spec("VERSIONING.md", "VERSIONING.md"),
            FileSpec {
                executable: true,
                ..spec("branding/build.py", "branding/build.py")
            },
        ],
        "library" => vec![spec("VERSIONING.md", "VERSIONING.md")],
        "cli-tool" => vec![
            spec("VERSIONING.md", "VERSIONING.md"),
            spec("build.rs", "rust/build.rs"),
        ],
        _ => Vec::new(),
    }
}

/// The identity from the arguments, checked so nothing odd lands in `.standards.conf`.
pub fn identity(
    product: &str,
    archetype: &str,
    lib: Option<&str>,
    upper: Option<&str>,
) -> Result<StandardsConf> {
    if !is_slug(product) {
        return Err(Error::Usage(format!(
            "`{product}` is not a product slug (letters, digits, `.`, `_`, `-`); pass --product"
        )));
    }
    let lib = lib.filter(|l| !l.is_empty());
    if let Some(lib) = lib
        && !is_slug(lib)
    {
        return Err(Error::Usage(format!("`{lib}` is not a crate name")));
    }
    let upper = match upper.filter(|u| !u.is_empty()) {
        Some(u)
            if u.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') =>
        {
            u.to_owned()
        }
        Some(u) => {
            return Err(Error::Usage(format!(
                "`{u}` is not an env prefix (A-Z, 0-9, `_`)"
            )));
        }
        None => StandardsConf::default_upper(product),
    };
    Ok(StandardsConf {
        product: product.to_owned(),
        product_upper: upper,
        lib: lib.map(str::to_owned),
        archetype: Some(archetype.to_owned()),
    })
}

fn is_slug(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A usage error unless the server knows `archetype`.
pub fn check_archetype(manifest: &Manifest, archetype: &str) -> Result<()> {
    if manifest.archetypes.iter().any(|a| a == archetype) {
        return Ok(());
    }
    Err(Error::Usage(format!(
        "unknown archetype `{archetype}`; one of: {}",
        manifest.archetypes.join(", ")
    )))
}

/// A file ready to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub dest: String,
    pub body: String,
    pub executable: bool,
    /// Placeholders the server left unfilled.
    pub unfilled: Vec<String>,
}

/// `.standards.conf` plus every spec, rendered with the identity's variables. Specs whose
/// destination already exists under `root` are left out (`skip_existing`), so `init` never
/// fetches what it would not write.
pub fn render(
    client: &Client,
    conf: &StandardsConf,
    specs: &[FileSpec],
    root: &Path,
) -> Result<Vec<Planned>> {
    let vars: BTreeMap<String, String> = conf.vars();
    let mut out = vec![Planned {
        dest: CONF_FILE.to_owned(),
        body: conf.to_conf_string(),
        executable: false,
        unfilled: Vec::new(),
    }];
    for spec in specs {
        if root.join(spec.dest).exists() {
            // Reported as skipped by `write`; no need to fetch it.
            out.push(Planned {
                dest: spec.dest.to_owned(),
                body: String::new(),
                executable: spec.executable,
                unfilled: Vec::new(),
            });
            continue;
        }
        let rendered = client.render_template(spec.template, &vars)?;
        let body = if spec.dest == "AGENTS.md" {
            strip_comment_blocks(&rendered.body)
        } else {
            rendered.body
        };
        out.push(Planned {
            dest: spec.dest.to_owned(),
            body,
            executable: spec.executable,
            unfilled: rendered.unfilled,
        });
    }
    Ok(out)
}

/// `bin/standards`' `sed -e '/^<!--/,/^-->/d' | sed '/./,$!d'`: drop every block from a
/// line starting `<!--` to the next line starting `-->` (an unclosed one runs to the end),
/// then the leading blank lines.
#[must_use]
pub fn strip_comment_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_block = false;
    let mut started = false;
    for line in text.split_inclusive('\n') {
        if in_block {
            if line.starts_with("-->") {
                in_block = false;
            }
            continue;
        }
        if line.starts_with("<!--") {
            // sed's range ends on a later line, never the opening one.
            in_block = true;
            continue;
        }
        if !started && line.trim_end_matches(['\n', '\r']).is_empty() {
            continue;
        }
        started = true;
        out.push_str(line);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Wrote,
    /// It was already there; left untouched.
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Written {
    pub path: String,
    pub action: Action,
    pub unfilled: Vec<String>,
}

/// Write each planned file under `root`, creating parents, never replacing one that exists.
pub fn write(root: &Path, planned: Vec<Planned>) -> Result<Vec<Written>> {
    let mut out = Vec::with_capacity(planned.len());
    for p in planned {
        let path = root.join(&p.dest);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        let action = match file {
            Ok(mut f) => {
                f.write_all(p.body.as_bytes())
                    .map_err(|e| Error::io(&path, e))?;
                if p.executable {
                    make_executable(&path)?;
                }
                Action::Wrote
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Action::Skipped,
            Err(e) => return Err(Error::io(&path, e)),
        };
        let unfilled = if action == Action::Wrote {
            p.unfilled
        } else {
            Vec::new()
        };
        out.push(Written {
            path: p.dest,
            action,
            unfilled,
        });
    }
    Ok(out)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| Error::io(path, e))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// The `-v` line naming the server and the content commit it served.
#[must_use]
pub fn server_line(client: &Client, manifest: &Manifest) -> String {
    format!("server {} (commit {})", client.host(), manifest.commit)
}

/// The outcome both commands report: the files, plus `extra` fields in the JSON.
#[must_use]
pub fn outcome(command: &str, root: &Path, conf: &StandardsConf, files: &[Written]) -> Outcome {
    let mut human = Vec::new();
    let mut warn = false;
    for f in files {
        match f.action {
            Action::Wrote => human.push(format!("wrote {}", f.path)),
            Action::Skipped => human.push(format!("skipped {} (exists)", f.path)),
        }
        if !f.unfilled.is_empty() {
            warn = true;
            human.push(format!(
                "  {} still has {}",
                f.path,
                f.unfilled
                    .iter()
                    .map(|k| format!("{{{{{k}}}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    let data: Value = json!({
        "command": command,
        "root": root.display().to_string(),
        "product": conf.product,
        "product_upper": conf.product_upper,
        "lib": conf.lib,
        "archetype": conf.archetype,
        "files": files,
    });
    Outcome {
        status: if warn { Status::Warn } else { Status::Ok },
        data,
        human,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_defaults_and_validation() {
        let c = identity("my-app", "library", None, None).unwrap();
        assert_eq!(c.product_upper, "MY_APP");
        assert_eq!(c.lib, None);
        let c = identity("x", "library", Some("libx"), Some("XX_1")).unwrap();
        assert_eq!(
            (c.lib.as_deref(), c.product_upper.as_str()),
            (Some("libx"), "XX_1")
        );
        let c = identity("x", "library", Some(""), Some("")).unwrap();
        assert_eq!((c.lib, c.product_upper.as_str()), (None, "X"));
        for (p, l, u) in [
            ("", None, None),
            ("-x", None, None),
            ("a b", None, None),
            ("a\nPRODUCT=b", None, None),
            ("x", Some("li b"), None),
            ("x", None, Some("lower")),
        ] {
            let e = identity(p, "library", l, u).unwrap_err();
            assert!(matches!(e, Error::Usage(_)), "{p:?} {l:?} {u:?}");
        }
    }

    #[test]
    fn readme_and_extras_per_archetype() {
        let readme = |a| base_files(a)[2].template;
        assert_eq!(readme("library"), "README.library.md");
        assert_eq!(readme("cli-tool"), "README.library.md");
        assert_eq!(readme("full-stack-product"), "README.product.md");
        assert_eq!(readme("orchestration"), "README.product.md");
        assert_eq!(extra_files("vendored-app"), []);
        assert_eq!(extra_files("orchestration"), []);
        assert_eq!(extra_files("cli-tool")[1].dest, "build.rs");
        assert!(extra_files("full-stack-product")[1].executable);
        assert_eq!(extra_files("library").len(), 1);
    }

    #[test]
    fn strips_like_the_sed_did() {
        let t = "<!--\nnote\n-->\n\n\nStandards: x\n\n<!-- one\nmore\n--> tail\nbody\n";
        assert_eq!(strip_comment_blocks(t), "Standards: x\n\nbody\n");
        assert_eq!(strip_comment_blocks("a\n<!-- open\nb\n"), "a\n");
        assert_eq!(strip_comment_blocks("plain"), "plain");
    }

    #[test]
    fn check_archetype_lists_choices() {
        let m = Manifest {
            schema: 1,
            commit: "c".into(),
            server_version: "0".into(),
            min_cli_version: "0".into(),
            cli_install_url: String::new(),
            archetypes: vec!["a".into(), "b".into()],
            skills: vec![],
        };
        assert!(check_archetype(&m, "a").is_ok());
        let e = check_archetype(&m, "c").unwrap_err().to_string();
        assert_eq!(e, "unknown archetype `c`; one of: a, b");
    }
}
