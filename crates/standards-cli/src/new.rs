//! `standards new <archetype> <product> [--lib] [--upper] [DIR]`: a new repo for an
//! archetype in an empty (or absent) directory, `./<product>` by default.
//!
//! It runs `git init -b main` (no commit), then writes `init`'s files plus the archetype's
//! small extra set ([`crate::scaffold`] lists it). Everything is rendered before the
//! directory is created, so a bad archetype or an unreachable server leaves nothing behind.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::output::{Outcome, Status};
use crate::{Ctx, scaffold};

#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// The archetype (`full-stack-product`, `library`, `cli-tool`, ...)
    pub archetype: String,
    /// Product slug
    pub product: String,
    /// The product's library crate, if it has one
    #[arg(long)]
    pub lib: Option<String>,
    /// Env prefix stem [default: PRODUCT upper-cased, `-` as `_`]
    #[arg(long, value_name = "ENV")]
    pub upper: Option<String>,
    /// Where to create it [default: ./<product>]
    pub dir: Option<PathBuf>,
}

pub fn run(ctx: &Ctx, args: &Args) -> Result<Outcome> {
    let conf = scaffold::identity(
        &args.product,
        &args.archetype,
        args.lib.as_deref(),
        args.upper.as_deref(),
    )?;
    let root = ctx.env.cwd.join(
        args.dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(&args.product)),
    );
    ensure_empty(&root)?;

    let client = ctx.client()?;
    let manifest = client.manifest()?;
    scaffold::check_archetype(&manifest, &args.archetype)?;

    let mut specs = scaffold::base_files(&args.archetype);
    specs.extend(scaffold::extra_files(&args.archetype));
    let planned = scaffold::render(&client, &conf, &specs, &root)?;

    std::fs::create_dir_all(&root).map_err(|e| Error::io(&root, e))?;
    let git = git_init(&root);
    let files = scaffold::write(&root, planned)?;

    let mut outcome = scaffold::outcome("new", &root, &conf, &files);
    outcome.data["git_init"] = serde_json::json!(git.is_ok());
    match git {
        Ok(()) => outcome
            .human
            .insert(0, format!("git init {}", root.display())),
        Err(reason) => {
            outcome.status = Status::Warn;
            outcome.human.push(format!("git init failed: {reason}"));
        }
    }
    if ctx.verbose {
        outcome
            .human
            .insert(0, scaffold::server_line(&client, &manifest));
    }
    Ok(outcome)
}

/// Absent, or an empty directory.
fn ensure_empty(root: &Path) -> Result<()> {
    match std::fs::read_dir(root) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(Error::Fail(format!(
                    "{} is not empty; for an existing repo use `standards init`",
                    root.display()
                )));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(root, e)),
    }
}

/// `git init -q -b main` in `root`. Commits are the user's; this only makes the repo.
fn git_init(root: &Path) -> std::result::Result<(), String> {
    let out = Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .arg(root)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_means_absent_or_no_entries() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ensure_empty(&dir.path().join("nope")).is_ok());
        assert!(ensure_empty(dir.path()).is_ok());
        std::fs::write(dir.path().join("f"), "").unwrap();
        assert!(ensure_empty(dir.path()).is_err());
        // A file where the directory should be.
        assert!(matches!(
            ensure_empty(&dir.path().join("f")),
            Err(Error::Io { .. })
        ));
    }

    #[test]
    fn git_init_reports_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "").unwrap();
        assert!(git_init(&file.join("sub")).is_err());
    }
}
