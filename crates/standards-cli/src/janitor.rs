//! `standards janitor`: the port of `bin/janitor`.
//!
//! TODO(STD-22): port `bin/janitor` behaviour-identically (SPECS.md "Existing bin/
//! behaviour to port exactly"): `janitor [scan|clean] [--root D].. [--days N] [--targets]
//! [--main-targets] [--branches] [-y]`. Replace [`Args`] (a subcommand enum can live here
//! too) and [`run`]; nothing outside this file needs to change. Write interactive output
//! through `io`; return an `Outcome` whose `data` is the `--json` shape.

use crate::output::{Io, Outcome};
use crate::{Ctx, Error, Result};

/// Placeholder: accepts anything until STD-22 defines the arguments.
#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    pub rest: Vec<String>,
}

pub fn run(_ctx: &Ctx, _args: &Args, _io: &mut Io<'_>) -> Result<Outcome> {
    Err(Error::NotImplemented("janitor"))
}
