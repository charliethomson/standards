//! The command line (L12):
//!
//! ```text
//! standards [--host URL] [--json] [-v] <doctor|init|new|self-update|janitor|ultron>
//! ```
//!
//! Each subcommand's arguments live beside its code (`init::Args`, `doctor::Args`, …), so
//! a task that fills one in edits only its own module.

use clap::{Parser, Subcommand};

use crate::{doctor, init, janitor, new, self_update, ultron};

#[derive(Debug, Parser)]
#[command(
    name = "standards",
    version = standards_core::VERSION,
    about = "Set repos up from the fleet standards and keep this machine in step with them.",
    propagate_version = true
)]
pub struct Cli {
    /// The standards server.
    #[arg(
        long,
        global = true,
        value_name = "URL",
        help = "The standards server [env: STANDARDS_HOST] [default: https://standards.thmsn.dev]"
    )]
    pub host: Option<String>,

    /// Print one JSON object instead of text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Say more.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check this machine and repo against the standards, and fix what drifted.
    Doctor(doctor::Args),
    /// Write .standards.conf, AGENTS.md, SHORTCUTS.md and a README into this repo.
    Init(init::Args),
    /// Scaffold a new repo for an archetype into an empty directory.
    New(new::Args),
    /// Update this cli from the registry.
    SelfUpdate(self_update::Args),
    /// Find and clean stale worktrees, branches and build output.
    Janitor(janitor::Args),
    /// Show the state of ultron programs.
    Ultron(ultron::Args),
}
