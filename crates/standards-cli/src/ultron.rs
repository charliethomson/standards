//! `standards ultron`: the port of `bin/ultron`.
//!
//! TODO(STD-23): port `bin/ultron` behaviour-identically (SPECS.md "Existing bin/
//! behaviour to port exactly"): `ultron [ls|status|watch|log|gc] ...`, state under
//! `${XDG_STATE_HOME:-~/.local/state}/thmsn/ultron/<slug>/`. Replace [`Args`] and [`run`];
//! nothing outside this file needs to change. `watch` draws straight to `io.out`; take the
//! state dir from `ctx.env` (add an `xdg_state_home` field there if needed) so tests never
//! read the real one.

use crate::output::{Io, Outcome};
use crate::{Ctx, Error, Result};

/// Placeholder: accepts anything until STD-23 defines the arguments.
#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    pub rest: Vec<String>,
}

pub fn run(_ctx: &Ctx, _args: &Args, _io: &mut Io<'_>) -> Result<Outcome> {
    Err(Error::NotImplemented("ultron"))
}
