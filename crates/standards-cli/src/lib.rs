//! The `standards` cli (`dev.thmsn.standards.cli`).
//!
//! Everything is here so tests drive it in-process; `main.rs` gathers the [`Env`] and
//! calls [`run`]. Layout:
//!
//! ```text
//! cli.rs          the clap surface (global --host/--json/-v, one variant per subcommand)
//! env.rs          Env: home, cache dir, cwd, current exe, platform, STANDARDS_* config
//! client.rs       Client: the server's REST mirror, typed errors (unreachable = exit 3)
//! cache.rs        ManifestCache: ~/.cache/thmsn/standards/manifest.json, hourly refresh
//! output.rs       Outcome + emit: --json object or terse lines; exit codes
//! error.rs        Error: exit code and $type per failure
//! repo.rs         git repo discovery
//! scaffold.rs     the file sets init/new write, rendered server-side, never clobbering
//! init.rs new.rs self_update.rs
//! doctor.rs janitor.rs ultron.rs   STD-21/22/23
//! ```
//!
//! The cli never commits.

pub mod cache;
pub mod cli;
pub mod client;
pub mod doctor;
pub mod env;
pub mod error;
pub mod init;
pub mod janitor;
pub mod new;
pub mod output;
pub mod repo;
pub mod scaffold;
pub mod self_update;
pub mod ultron;

#[cfg(test)]
mod testing;

use std::ffi::OsString;
use std::time::Duration;

use clap::Parser as _;
use clap::error::ErrorKind;

pub use crate::cli::{Cli, Command};
pub use crate::client::Client;
pub use crate::env::Env;
pub use crate::error::{Error, Result, exit};
pub use crate::output::{Io, Outcome, Status};

/// What every command gets.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// The resolved server (`--host`, `STANDARDS_HOST`, default).
    pub host: String,
    pub json: bool,
    pub verbose: bool,
    pub env: Env,
}

impl Ctx {
    /// A client for the server with the ordinary timeout.
    pub fn client(&self) -> Result<Client> {
        self.client_with_timeout(client::DEFAULT_TIMEOUT)
    }

    pub fn client_with_timeout(&self, timeout: Duration) -> Result<Client> {
        Ok(Client::new(&self.host, timeout)?)
    }
}

/// Parse `args` (including the program name), run the command, print, and return the exit
/// code.
pub fn run<I, T>(args: I, env: Env, io: &mut Io<'_>) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let help = matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion);
            let rendered = e.render().to_string();
            if help {
                let _ = write!(io.out, "{rendered}");
                return exit::OK;
            }
            let _ = write!(io.err, "{rendered}");
            return exit::USAGE;
        }
    };
    let ctx = Ctx {
        host: env.host(cli.host.as_deref()),
        json: cli.json,
        verbose: cli.verbose,
        env,
    };
    let result = dispatch(&ctx, &cli.command, io);
    output::emit(io, ctx.json, result)
}

fn dispatch(ctx: &Ctx, command: &Command, io: &mut Io<'_>) -> Result<Outcome> {
    match command {
        Command::Doctor(args) => doctor::run(ctx, args, io),
        Command::Init(args) => init::run(ctx, args),
        Command::New(args) => new::run(ctx, args),
        Command::SelfUpdate(args) => self_update::run(ctx, args),
        Command::Janitor(args) => janitor::run(ctx, args, io),
        Command::Ultron(args) => ultron::run(ctx, args, io),
    }
}
