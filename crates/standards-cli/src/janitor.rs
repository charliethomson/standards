//! `standards janitor`: find (and clean up) the git worktrees and cargo target dirs agents
//! leave behind. The port of `bin/janitor` (L16), behaviour for behaviour:
//!
//! ```text
//! standards janitor [scan]  [options]   report every linked worktree and what `clean` would do
//! standards janitor clean   [options]   do it, after one confirmation prompt
//! ```
//!
//! A worktree is only removed when losing it can't lose work: no uncommitted or untracked
//! changes, not locked, no process has its cwd inside it, idle for `--days`, and its HEAD is
//! either merged into the default branch or still held by a branch (removing a worktree
//! never deletes its branch). Detached worktrees with unmerged commits are reported and kept.
//!
//! Like the script, it shells out to `git`, `lsof` and `du`. Layout:
//!
//! ```text
//! janitor/sys.rs     the machine: git, lsof, du, mtimes, repo and target discovery
//! janitor/scan.rs    the report: per-repo worktrees, verdicts and the plan
//! janitor/render.rs  the human lines, as bin/janitor prints them
//! janitor/clean.rs   carrying out the plan
//! ```

mod clean;
mod render;
mod scan;
mod sys;
#[cfg(test)]
mod tests;

use std::io::{BufRead as _, IsTerminal as _};
use std::path::PathBuf;

use serde_json::json;

use crate::output::{Io, Outcome, Status};
use crate::{Ctx, Error, Result};

pub use scan::{Action, PlanItem, Report, Verdict};

/// What to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Report every linked worktree and what `clean` would do (read-only).
    #[default]
    Scan,
    /// Do it, after one confirmation prompt.
    Clean,
}

// One bool per bin/janitor switch: the flags are the interface being ported.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// `scan` (the default) or `clean`.
    #[arg(value_enum)]
    pub mode: Option<Mode>,

    /// Where to look for repos (repeatable; default ~/git).
    #[arg(long = "root", value_name = "DIR")]
    pub roots: Vec<PathBuf>,

    /// A worktree untouched for fewer days is left alone.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_DAYS)]
    pub days: u64,

    /// Also delete cargo target dirs inside worktrees that are kept (dirty, unmerged,
    /// recent); target dirs are always safe to rebuild.
    #[arg(long)]
    pub targets: bool,

    /// Also delete target dirs in main checkouts idle for --days or more.
    #[arg(long)]
    pub main_targets: bool,

    /// After removing a merged worktree, delete its branch too (git branch -d).
    #[arg(long)]
    pub branches: bool,

    /// Skip the confirmation prompt.
    #[arg(short, long)]
    pub yes: bool,
}

/// The defaults the flags have: scan, `~/git`, 3 days, nothing extra.
impl Default for Args {
    fn default() -> Self {
        Self {
            mode: None,
            roots: Vec::new(),
            days: DEFAULT_DAYS,
            targets: false,
            main_targets: false,
            branches: false,
            yes: false,
        }
    }
}

const DEFAULT_DAYS: u64 = 3;

/// The terminal around a run: whether to colour, and how to ask before cleaning.
pub trait Term {
    /// Colour the human output (bin/janitor: when stdout is a terminal).
    fn color(&self) -> bool;
    /// Whether stdin is a terminal someone can answer a prompt on.
    fn stdin_is_terminal(&self) -> bool;
    /// Read one answer line; `None` at end of input.
    fn read_answer(&mut self) -> Option<String>;
}

/// The real process terminal.
struct ProcessTerm;

impl Term for ProcessTerm {
    fn color(&self) -> bool {
        std::io::stdout().is_terminal()
    }

    fn stdin_is_terminal(&self) -> bool {
        std::io::stdin().is_terminal()
    }

    fn read_answer(&mut self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line),
        }
    }
}

pub fn run(ctx: &Ctx, args: &Args, io: &mut Io<'_>) -> Result<Outcome> {
    run_with(ctx, args, io, &mut ProcessTerm, &sys::live_cwds())
}

/// [`run`] with the terminal and the live process cwds given, so tests drive both.
pub fn run_with(
    ctx: &Ctx,
    args: &Args,
    io: &mut Io<'_>,
    term: &mut dyn Term,
    live_cwds: &[String],
) -> Result<Outcome> {
    let mode = args.mode.unwrap_or_default();
    let palette = render::Palette::new(term.color() && !ctx.json);
    let roots: Vec<PathBuf> = if args.roots.is_empty() {
        vec![ctx.env.home.join("git")]
    } else {
        args.roots.iter().map(|r| ctx.env.cwd.join(r)).collect()
    };
    for root in roots.iter().filter(|r| !r.is_dir()) {
        let _ = writeln!(
            io.err,
            "{}warning:{} no such root {}",
            palette.yel,
            palette.off,
            root.display()
        );
    }
    let repos = sys::find_repos(&roots);
    if repos.is_empty() {
        let joined: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
        return Err(Error::Fail(format!(
            "no git repos under {}",
            joined.join(" ")
        )));
    }

    let opts = scan::Options {
        days: args.days,
        main_targets: args.main_targets,
        home: ctx.env.home.clone(),
        now: sys::now(),
    };
    let report = scan::scan(&repos, &opts, live_cwds);
    let summary = report.summary(args.targets, args.main_targets);

    if !ctx.json {
        render::report(io.out, &report, &palette);
        let _ = writeln!(io.out, "{}", render::summary(&summary, &palette));
    }
    let mut data = json!({
        "mode": mode,
        "roots": roots,
        "days": args.days,
        "repos": report.repos,
        "plan": report.plan,
        "summary": summary,
    });

    if mode == Mode::Scan {
        if !ctx.json {
            let _ = writeln!(
                io.out,
                "{}run `standards janitor clean` (same options) to do it{}",
                palette.dim, palette.off
            );
        }
        return Ok(Outcome::ok(data));
    }

    let plan: Vec<PlanItem> = report
        .plan
        .into_iter()
        .filter(|p| args.targets || p.action != Action::Targets)
        .collect();
    if plan.is_empty() {
        if !ctx.json {
            let _ = writeln!(io.out, "nothing to do");
        }
        data["clean"] = json!({ "done": false, "reason": "nothing to do" });
        return Ok(Outcome::ok(data));
    }

    if !args.yes {
        if ctx.json {
            return Err(Error::Fail(
                "--json can't prompt; pass --yes to clean".to_owned(),
            ));
        }
        if !term.stdin_is_terminal() {
            return Err(Error::Fail(
                "not a terminal; pass --yes to clean without a prompt".to_owned(),
            ));
        }
        let _ = write!(io.out, "go ahead? [y/N] ");
        let _ = io.out.flush();
        let answer = term.read_answer().unwrap_or_default();
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            let _ = writeln!(io.out, "nothing changed");
            data["clean"] = json!({ "done": false, "reason": "declined" });
            return Ok(Outcome::ok(data));
        }
    }

    let result = clean::execute(&plan, args.branches);
    if !ctx.json {
        render::clean(io.out, &result, &palette);
    }
    let status = if result.skipped() {
        Status::Warn
    } else {
        Status::Ok
    };
    data["clean"] =
        json!({ "done": true, "results": result.results, "freed_kib": result.freed_kib });
    let mut outcome = Outcome::ok(data);
    outcome.status = status;
    Ok(outcome)
}
