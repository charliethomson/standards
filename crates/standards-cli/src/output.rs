//! What a command hands back, and how it is printed.
//!
//! A command returns an [`Outcome`]: a status, JSON data, and terse human lines. With
//! `--json` the whole stdout is one object, `{"status": "ok"|"warn"|"fail", "data": …}`, or
//! for an error `{"status": "error", "error": {"message", "$type", "exit_code"}}`. Without it
//! the human lines go to stdout and an error is one `error: …` line on stderr.

use std::io::Write;

use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{Error, exit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    /// Done, with something worth a look. Still exit 0.
    Warn,
    /// The command ran and found a failure (doctor). Exit 1.
    Fail,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub status: Status,
    pub data: Value,
    pub human: Vec<String>,
}

impl Outcome {
    #[must_use]
    pub fn ok(data: Value) -> Self {
        Self {
            status: Status::Ok,
            data,
            human: Vec::new(),
        }
    }

    #[must_use]
    pub fn line(mut self, line: impl Into<String>) -> Self {
        self.human.push(line.into());
        self
    }

    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.status {
            Status::Ok | Status::Warn => exit::OK,
            Status::Fail => exit::FAIL,
        }
    }
}

/// Where output goes. `main` passes the real stdout/stderr; tests pass buffers.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// Print a finished command and return its exit code.
pub fn emit(io: &mut Io<'_>, json_mode: bool, result: Result<Outcome, Error>) -> u8 {
    match result {
        Ok(outcome) => {
            if json_mode {
                let v = json!({ "status": outcome.status, "data": outcome.data });
                let _ = writeln!(io.out, "{v}");
            } else {
                for line in &outcome.human {
                    let _ = writeln!(io.out, "{line}");
                }
            }
            outcome.exit_code()
        }
        Err(error) => {
            let code = error.exit_code();
            if json_mode {
                let v = json!({
                    "status": "error",
                    "error": {
                        "message": error.to_string(),
                        "$type": error.key(),
                        "exit_code": code,
                    }
                });
                let _ = writeln!(io.out, "{v}");
            } else {
                let _ = writeln!(io.err, "error: {error}");
            }
            code
        }
    }
}
