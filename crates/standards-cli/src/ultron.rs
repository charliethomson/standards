//! `standards ultron`: the port of `bin/ultron`, a read-only view of in-flight ultron
//! programs under `${XDG_STATE_HOME:-~/.local/state}/thmsn/ultron/<slug>/`.
//!
//! ```text
//! standards ultron [ls]                       programs: task counts, blocked, gated, last activity
//! standards ultron status [<program>]         per-task table (default: the most recent program)
//! standards ultron watch [-n S] [<program>]   live status, redrawn when the state changes
//! standards ultron log <TASK-ID>              print a task's journal
//! standards ultron gc <program>               delete a program's state (always prompts)
//! ```
//!
//! Output is byte-identical to the script (golden tests in `ultron/tests.rs` compare the
//! two over real programs). The arguments are parsed here rather than by clap so the
//! aliases (`list`, `st`, `w`, `cat`), the error messages and exit 1 on a bad argument stay
//! the script's. `--json` (before `ultron`) is the one addition: one object per command.
//!
//! ```text
//! ultron/backlog.rs   PROGRAM.md -> tasks; PLAN progress; the STATE line (pure, over bytes)
//! ultron/state.rs     the state dir: programs by recency, newest mtime, fingerprint, ago()
//! ultron/view.rs      ls / status: summaries (the --json shape) and their rendering
//! ultron/watch.rs     watch: alt screen, redraw on fingerprint change, wait for a program
//! ultron/host.rs      clock, stdin and interruptible sleep, swapped out in tests
//! ```

mod backlog;
mod host;
mod state;
mod view;
mod watch;

#[cfg(test)]
mod tests;

use std::io::Write;

use serde_json::{Value, json};

use self::host::{Host, RealHost};
use self::state::{StateDir, die};
use self::view::{Style, program_status, render_ls, render_status, summarize};
use self::watch::{WatchArgs, Watcher};
use crate::output::{Io, Outcome};
use crate::{Ctx, Error, Result};

/// The script's own help: its header comment.
const USAGE: &str = "\
ultron \u{2014} read-only view of in-flight Ultron programs.

  ultron ls                  list programs: task counts, what's blocked, last activity
  ultron status [<program>]  per-task table for a program (default: the only/most recent one)
  ultron watch [<program>]   live status, redrawn when the state changes (-n <seconds>)
  ultron log <TASK-ID>       print a task's journal
  ultron gc [<program>]      remove state for a fully-landed program (prompts)

Reads the same state Ultron writes, so checking where a program stands costs no tokens.
vision is the durable record; everything here is reconstructible scratch.
";

#[derive(Debug, Clone, clap::Args)]
#[command(disable_help_flag = true)]
pub struct Args {
    /// `ls` (default), `status [<program>]`, `watch [-n <seconds>] [<program>]`,
    /// `log <TASK-ID>`, `gc <program>`, `help`.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "COMMAND"
    )]
    pub args: Vec<String>,
}

pub fn run(ctx: &Ctx, args: &Args, io: &mut Io<'_>) -> Result<Outcome> {
    run_with(ctx, args, io, &mut RealHost)
}

fn stdout(e: std::io::Error) -> Error {
    Error::io(std::path::Path::new("<stdout>"), e)
}

/// Text was already written to `io.out`; with `--json`, `data` is the whole output.
fn done(data: Value) -> Outcome {
    Outcome::ok(data)
}

pub(crate) fn run_with(
    ctx: &Ctx,
    args: &Args,
    io: &mut Io<'_>,
    host: &mut dyn Host,
) -> Result<Outcome> {
    let state = StateDir::new(ctx.env.ultron_state_dir());
    let style = Style::new(ctx.env.stdout_tty && !ctx.json);
    let words: Vec<&str> = args.args.iter().map(String::as_str).collect();
    let (cmd, rest) = words.split_first().map_or(("", &[][..]), |(c, r)| (*c, r));
    // `${1:-}`: an empty argument is the same as none.
    let first = rest.first().copied().filter(|a| !a.is_empty());

    match cmd {
        "" | "ls" | "list" => {
            let rows = summarize(&state, host.now());
            if !ctx.json {
                render_ls(io.out, &state, &rows, style).map_err(stdout)?;
            }
            Ok(done(json!({ "state_dir": state.root(), "programs": rows })))
        }
        "status" | "st" => status(ctx, &state, first, io, host, style),
        "watch" | "w" => {
            let wargs = WatchArgs::parse(rest)?;
            // Validate before taking over the screen, so the error is readable.
            if let Some(slug) = &wargs.slug {
                state.program_dir(slug)?;
            }
            // Not a terminal (piped, redirected, CI) or --json: print once and stop.
            if !ctx.env.stdout_tty || ctx.json {
                return status(ctx, &state, wargs.slug.as_deref(), io, host, style);
            }
            Watcher::new(&state, wargs, style).run(io.out, host)?;
            Ok(done(Value::Null))
        }
        "log" | "cat" => log(ctx, &state, first, io, style),
        "gc" => gc(ctx, &state, first, io, host, style),
        "-h" | "--help" | "help" => {
            if !ctx.json {
                io.out.write_all(USAGE.as_bytes()).map_err(stdout)?;
            }
            Ok(done(json!({ "usage": USAGE })))
        }
        other => Err(die(format!(
            "unknown command: {other} (try 'ultron --help')"
        ))),
    }
}

fn status(
    ctx: &Ctx,
    state: &StateDir,
    slug: Option<&str>,
    io: &mut Io<'_>,
    host: &dyn Host,
    style: Style,
) -> Result<Outcome> {
    let slug = match slug {
        Some(s) => s.to_owned(),
        None => state.default_program()?,
    };
    let dir = state.program_dir(&slug)?;
    let st = program_status(&slug, &dir, host.now());
    if !ctx.json {
        render_status(io.out, &st, style).map_err(stdout)?;
    }
    Ok(done(serde_json::to_value(&st).unwrap_or_default()))
}

fn log(
    ctx: &Ctx,
    state: &StateDir,
    id: Option<&str>,
    io: &mut Io<'_>,
    s: Style,
) -> Result<Outcome> {
    let id = id.ok_or_else(|| die("usage: ultron log <TASK-ID>"))?;
    let hits = state.journals_named(id);
    if hits.is_empty() {
        return Err(die(format!("no journal for {id}")));
    }
    let mut journals = Vec::new();
    for path in hits {
        let body = std::fs::read(&path).map_err(|e| Error::io(path.as_ref(), e))?;
        if !ctx.json {
            writeln!(io.out, "{}\u{2500}\u{2500} {path}{}", s.dim, s.off).map_err(stdout)?;
            io.out.write_all(&body).map_err(stdout)?;
            if !body.is_empty() && !body.ends_with(b"\n") {
                writeln!(io.out).map_err(stdout)?;
            }
        }
        journals.push(json!({
            "path": path,
            "content": String::from_utf8_lossy(&body),
        }));
    }
    Ok(done(json!({ "journals": journals })))
}

fn gc(
    ctx: &Ctx,
    state: &StateDir,
    slug: Option<&str>,
    io: &mut Io<'_>,
    host: &mut dyn Host,
    s: Style,
) -> Result<Outcome> {
    let slug = slug.ok_or_else(|| {
        die("usage: ultron gc <program>   (name it explicitly \u{2014} this deletes state)")
    })?;
    let dir = state.program_dir(slug)?;
    let tasks = view::tasks_in(&dir);
    let total = tasks.len();
    let landed = tasks
        .iter()
        .filter(|t| t.state == backlog::TaskState::Landed)
        .count();
    // The prompt goes where a human reads it: stderr under --json, so stdout stays one object.
    let out: &mut dyn Write = if ctx.json { &mut *io.err } else { &mut *io.out };
    if total == 0 || landed != total {
        writeln!(
            out,
            "{}! {slug} is {landed}/{total} landed \u{2014} not finished.{}",
            s.yel, s.off
        )
        .map_err(stdout)?;
    }
    write!(out, "delete {dir} ? [y/N] ").map_err(stdout)?;
    out.flush().map_err(stdout)?;
    let reply = host.read_line().unwrap_or_default();
    let removed = matches!(reply.as_str(), "y" | "Y");
    if removed {
        StateDir::remove(&dir)?;
        writeln!(out, "{}\u{2713}{} removed {slug}", s.grn, s.off).map_err(stdout)?;
    } else {
        writeln!(out, "kept.").map_err(stdout)?;
    }
    Ok(done(json!({
        "slug": slug,
        "dir": dir,
        "landed": landed,
        "total": total,
        "removed": removed,
    })))
}
