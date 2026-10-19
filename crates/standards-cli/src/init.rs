//! `standards init [--product] [--archetype] [--lib] [--upper]`: the port of
//! `bin/standards install` without the skill symlinks and `.mcp.json` (the MCP and the
//! skills are per machine now, `standards doctor`).
//!
//! Run anywhere inside an existing git repo; it writes at the repo root. Identity comes
//! from the flags, else an existing `.standards.conf`, else the repo directory's name and
//! `full-stack-product`. Never overwrites: running it again in an older repo backfills
//! whatever is missing. Never commits or stages.

use std::path::Path;

use standards_core::StandardsConf;

use crate::error::{Error, Result};
use crate::output::Outcome;
use crate::{Ctx, repo, scaffold};

#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// Product slug [default: the repo directory's name]
    #[arg(long)]
    pub product: Option<String>,
    /// Archetype [default: full-stack-product]
    #[arg(long)]
    pub archetype: Option<String>,
    /// The product's library crate, if it has one
    #[arg(long)]
    pub lib: Option<String>,
    /// Env prefix stem [default: PRODUCT upper-cased, `-` as `_`]
    #[arg(long, value_name = "ENV")]
    pub upper: Option<String>,
}

pub fn run(ctx: &Ctx, args: &Args) -> Result<Outcome> {
    let root = repo::find_root(&ctx.env.cwd).ok_or_else(|| {
        Error::Fail(format!(
            "{} is not in a git repo; for a new one use `standards new`",
            ctx.env.cwd.display()
        ))
    })?;
    let existing = read_conf(&root)?;
    let conf = resolve_identity(args, existing.as_ref(), &root)?;

    let client = ctx.client()?;
    let manifest = client.manifest()?;
    let archetype = conf.archetype.as_deref().unwrap_or_default();
    scaffold::check_archetype(&manifest, archetype)?;

    let planned = scaffold::render(&client, &conf, &scaffold::base_files(archetype), &root)?;
    let files = scaffold::write(&root, planned)?;
    let mut outcome = scaffold::outcome("init", &root, &conf, &files);
    if ctx.verbose {
        outcome
            .human
            .insert(0, scaffold::server_line(&client, &manifest));
    }
    Ok(outcome)
}

/// Flags win; anything not given comes from the repo's `.standards.conf`, then the
/// defaults.
fn resolve_identity(
    args: &Args,
    existing: Option<&StandardsConf>,
    root: &Path,
) -> Result<StandardsConf> {
    let dir_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let product = args
        .product
        .clone()
        .or_else(|| existing.map(|c| c.product.clone()))
        .unwrap_or(dir_name);
    let archetype = args
        .archetype
        .clone()
        .or_else(|| existing.and_then(|c| c.archetype.clone()))
        .unwrap_or_else(|| scaffold::DEFAULT_ARCHETYPE.to_owned());
    // An existing conf's upper/lib only apply to its own product.
    let same = existing.filter(|c| c.product == product);
    let lib = args
        .lib
        .clone()
        .or_else(|| same.and_then(|c| c.lib.clone()));
    let upper = args
        .upper
        .clone()
        .or_else(|| same.map(|c| c.product_upper.clone()));
    scaffold::identity(&product, &archetype, lib.as_deref(), upper.as_deref())
}

/// The repo's `.standards.conf`, if it has one. One that does not parse is an error, not a
/// reason to write another.
fn read_conf(root: &Path) -> Result<Option<StandardsConf>> {
    let path = root.join(scaffold::CONF_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => StandardsConf::parse(&text)
            .map(Some)
            .map_err(|e| Error::Fail(format!("{e}; fix it or remove it, then rerun"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(&path, e)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn args() -> Args {
        Args {
            product: None,
            archetype: None,
            lib: None,
            upper: None,
        }
    }

    #[test]
    fn identity_precedence() {
        let root = PathBuf::from("/x/my-repo");
        let c = resolve_identity(&args(), None, &root).unwrap();
        assert_eq!(c.product, "my-repo");
        assert_eq!(c.product_upper, "MY_REPO");
        assert_eq!(c.archetype.as_deref(), Some("full-stack-product"));

        let existing = StandardsConf::parse(
            "PRODUCT=vision\nPRODUCT_UPPER=VIS\nLIB=libvision\nARCHETYPE=library\n",
        )
        .unwrap();
        let c = resolve_identity(&args(), Some(&existing), &root).unwrap();
        assert_eq!(c, existing);

        let a = Args {
            product: Some("other".into()),
            archetype: Some("cli-tool".into()),
            ..args()
        };
        let c = resolve_identity(&a, Some(&existing), &root).unwrap();
        assert_eq!(c.product_upper, "OTHER");
        assert_eq!(c.lib, None);
        assert_eq!(c.archetype.as_deref(), Some("cli-tool"));
    }

    #[test]
    fn unparsable_conf_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".standards.conf"), "garbage\n").unwrap();
        let e = read_conf(dir.path()).unwrap_err();
        assert!(e.to_string().contains("fix it or remove it"), "{e}");
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_conf(dir.path()).unwrap(), None);
    }
}
