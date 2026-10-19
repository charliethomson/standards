//! `standards doctor`: user-scope and repo-scope checks with `--fix`/`--quiet`/`--ci`.
//!
//! TODO(STD-21): implement per SPECS.md "CLI contract" (`doctor [--fix] [--quiet] [--ci]
//! [--only ID..] [--repo PATH]`). Replace [`Args`] with the real flags and [`run`] with the
//! checks; nothing outside this file needs to change. Building blocks already in place:
//! `ctx.client()` / `ctx.client_with_timeout(cache::QUIET_TIMEOUT)`,
//! `cache::ManifestCache::for_env(&ctx.env).load(..)` for `--quiet`, `repo::find_root`,
//! `ctx.env.home` (never the real `$HOME` in tests), and `Outcome` with `Status::Warn` /
//! `Status::Fail` for exit 0 / 1. `io` is there for streaming output if a check wants it.

use crate::output::{Io, Outcome};
use crate::{Ctx, Error, Result};

/// Placeholder: accepts anything until STD-21 defines the flags.
#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    pub rest: Vec<String>,
}

pub fn run(_ctx: &Ctx, _args: &Args, _io: &mut Io<'_>) -> Result<Outcome> {
    Err(Error::NotImplemented("doctor"))
}
