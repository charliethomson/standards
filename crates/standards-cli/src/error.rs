//! Every failure a command can end in, and the exit code and error key it maps to
//! (docs/error-handling.md).
//!
//! Exit codes are contract (L12): `0` ok or warn, `1` fail, `2` usage, `3` the server (or
//! registry) was needed and could not be reached.

use std::path::{Path, PathBuf};

use crate::client::ClientError;

/// Exit codes.
pub mod exit {
    pub const OK: u8 = 0;
    pub const FAIL: u8 = 1;
    pub const USAGE: u8 = 2;
    pub const UNREACHABLE: u8 = 3;
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A bad argument the parser could not catch (an unknown archetype, a bad slug).
    #[error("{0}")]
    Usage(String),
    /// The standards server.
    #[error(transparent)]
    Client(#[from] ClientError),
    /// The registry, for `self-update`.
    #[error(transparent)]
    Registry(#[from] crate::self_update::RegistryError),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The command cannot go on (`not in a git repo`, `directory not empty`).
    #[error("{0}")]
    Fail(String),
    /// A subcommand whose task has not landed yet.
    #[error("{0}: not implemented yet")]
    NotImplemented(&'static str),
}

impl Error {
    /// An I/O error on `path`.
    #[must_use]
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => exit::USAGE,
            Self::Client(e) if e.is_unreachable() => exit::UNREACHABLE,
            Self::Registry(e) if e.is_unreachable() => exit::UNREACHABLE,
            _ => exit::FAIL,
        }
    }

    /// The stable error key (`dev.thmsn.standards.cli.<area>.error.<kind>`).
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Usage(_) => "dev.thmsn.standards.cli.args.error.usage",
            Self::Client(e) => e.key(),
            Self::Registry(e) => e.key(),
            Self::Io { .. } => "dev.thmsn.standards.cli.fs.error.io",
            Self::Fail(_) => "dev.thmsn.standards.cli.command.error.failed",
            Self::NotImplemented(_) => "dev.thmsn.standards.cli.command.error.not_implemented",
        }
    }
}
