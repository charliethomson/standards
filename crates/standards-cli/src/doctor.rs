//! `standards doctor [--fix] [--quiet] [--ci] [--only ID..] [--repo PATH]`: check this
//! machine and the current repo against the standards server, and fix what drifted.
//!
//! Two scopes, run in order. **User scope** (skipped with `--ci`) wires the machine: the
//! MCP registration, skill stubs, the `SessionStart` hook, the cli's own version, old
//! `~/.local/bin` links. **Repo scope** (skipped outside a git repo) checks the repo:
//! `.standards.conf`, the `AGENTS.md` line, and the leftovers of the `standards/`
//! submodule. Each check is one row of [`CHECKS`]; `--only` picks rows by id.
//!
//! - `--fix` applies each failing check's fix, then runs the check again and reports what
//!   it finds. Repo changes are staged (`git add` / `git rm`), never committed.
//! - `--quiet` is the `SessionStart` hook: the manifest comes from the cache (refreshed at
//!   most hourly with a 1s budget), nothing is fixed, the registry is not asked about a
//!   newer cli, and the only output is one line when something needs attention. It always
//!   exits 0.
//! - `--ci` runs repo scope only; a failing repo check exits 1.
//! - Exit 3 when a selected check needs the manifest and the server can't be reached.
//!
//! A git repo that has no `.standards.conf` and none of the submodule leftovers is not a
//! standards repo: outside `--ci` its repo checks are skipped, so the hook stays quiet in
//! repos that never opted in.

mod files;
mod repo;
mod user;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde_json::json;
use standards_core::Manifest;

use crate::cache::{self, ManifestCache};
use crate::error::{Error, Result};
use crate::output::{Io, Outcome, Status};
use crate::{Ctx, repo as repo_root};

/// The budget for `mcp.reachable` and the manifest fetch outside `--quiet`.
pub const REACH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// Repair what drifted (repo changes are staged, never committed)
    #[arg(long, conflicts_with = "quiet")]
    pub fix: bool,
    /// For the `SessionStart` hook: cached manifest, no fixes, one line only when something
    /// needs attention, always exit 0
    #[arg(long)]
    pub quiet: bool,
    /// Repo scope only; exit 1 when a repo check fails
    #[arg(long)]
    pub ci: bool,
    /// Run only these checks (comma or space separated ids)
    #[arg(long, value_name = "ID", num_args = 1.., value_delimiter = ',')]
    pub only: Vec<String>,
    /// The repo to check [default: the one containing the current directory]
    #[arg(long, value_name = "PATH")]
    pub repo: Option<PathBuf>,
}

/// Which half of the run a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    Repo,
}

/// A check's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Skip,
    Ok,
    Warn,
    Fail,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub message: String,
    pub details: Vec<String>,
    pub fixable: bool,
}

impl Finding {
    fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            details: Vec::new(),
            fixable: false,
        }
    }

    pub fn ok(message: impl Into<String>) -> Self {
        Self::new(Level::Ok, message)
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(Level::Warn, message)
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self::new(Level::Fail, message)
    }

    pub fn skip(message: impl Into<String>) -> Self {
        Self::new(Level::Skip, message)
    }

    #[must_use]
    pub fn fixable(mut self, fixable: bool) -> Self {
        self.fixable = fixable;
        self
    }

    #[must_use]
    pub fn details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

/// A check as reported: `{id, scope, status, message, fixable, fixed, details}`.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub id: &'static str,
    pub scope: Scope,
    pub status: Level,
    pub message: String,
    pub fixable: bool,
    /// `--fix` ran this check's fix.
    pub fixed: bool,
    pub details: Vec<String>,
}

/// What every check reads.
pub struct Cx<'a> {
    pub ctx: &'a Ctx,
    /// The server, without a trailing slash.
    pub host: String,
    pub manifest: Option<Manifest>,
    /// How the manifest fetch went: the round trip, or why it failed. `None` when it was
    /// not attempted (`--quiet`, or no selected check needed it).
    pub reach: Option<std::result::Result<Duration, String>>,
    pub quiet: bool,
    pub ci: bool,
    /// The repo root, for repo scope.
    pub repo: Option<PathBuf>,
}

impl Cx<'_> {
    fn home(&self) -> &Path {
        &self.ctx.env.home
    }

    /// The repo root; repo checks only run with one.
    fn root(&self) -> &Path {
        self.repo.as_deref().unwrap_or(Path::new("."))
    }
}

type CheckFn = fn(&Cx<'_>) -> Finding;
type FixFn = fn(&Cx<'_>) -> Result<Vec<String>>;

/// One row of the check table.
struct Def {
    id: &'static str,
    scope: Scope,
    /// Needs the manifest: unreachable means exit 3 (outside `--quiet`).
    needs_manifest: bool,
    check: CheckFn,
    fix: Option<FixFn>,
    /// Run the check again after a fix. Off for `cli.current`, whose fix replaces a binary
    /// this process can't re-read.
    recheck: bool,
}

/// Every check, in the order they run (SPECS.md "Doctor checks").
const CHECKS: &[Def] = &[
    Def {
        id: "mcp.registered",
        scope: Scope::User,
        needs_manifest: false,
        check: user::mcp_registered,
        fix: Some(user::fix_mcp_registered),
        recheck: true,
    },
    Def {
        id: "mcp.reachable",
        scope: Scope::User,
        needs_manifest: false,
        check: user::mcp_reachable,
        fix: None,
        recheck: true,
    },
    Def {
        id: "skills.synced",
        scope: Scope::User,
        needs_manifest: true,
        check: user::skills_synced,
        fix: Some(user::fix_skills_synced),
        recheck: true,
    },
    Def {
        id: "hook.session-start",
        scope: Scope::User,
        needs_manifest: false,
        check: user::hook_session_start,
        fix: Some(user::fix_hook_session_start),
        recheck: true,
    },
    Def {
        id: "cli.current",
        scope: Scope::User,
        needs_manifest: true,
        check: user::cli_current,
        fix: Some(user::fix_cli_current),
        recheck: false,
    },
    Def {
        id: "legacy.path-links",
        scope: Scope::User,
        needs_manifest: false,
        check: user::path_links,
        fix: Some(user::fix_path_links),
        recheck: true,
    },
    Def {
        id: "repo.conf",
        scope: Scope::Repo,
        needs_manifest: true,
        check: repo::conf,
        fix: None,
        recheck: true,
    },
    Def {
        id: "repo.agents",
        scope: Scope::Repo,
        needs_manifest: false,
        check: repo::agents,
        fix: Some(repo::fix_agents),
        recheck: true,
    },
    Def {
        id: "legacy.submodule",
        scope: Scope::Repo,
        needs_manifest: false,
        check: repo::submodule,
        fix: Some(repo::fix_submodule),
        recheck: true,
    },
    Def {
        id: "legacy.skill-links",
        scope: Scope::Repo,
        needs_manifest: false,
        check: repo::skill_links,
        fix: Some(repo::fix_skill_links),
        recheck: true,
    },
    Def {
        id: "legacy.mcp-json",
        scope: Scope::Repo,
        needs_manifest: false,
        check: repo::mcp_json,
        fix: Some(repo::fix_mcp_json),
        recheck: true,
    },
    Def {
        id: "legacy.ci-override",
        scope: Scope::Repo,
        needs_manifest: false,
        check: repo::ci_override,
        fix: None,
        recheck: true,
    },
];

/// The check ids, in run order.
#[must_use]
pub fn check_ids() -> Vec<&'static str> {
    CHECKS.iter().map(|d| d.id).collect()
}

pub fn run(ctx: &Ctx, args: &Args, _io: &mut Io<'_>) -> Result<Outcome> {
    if args.quiet {
        // The hook must never get in the way of a session: whatever happens, exit 0.
        return Ok(match run_checks(ctx, args) {
            Ok(run) => quiet_outcome(&run),
            Err(_) => Outcome::ok(json!({ "checks": [] })),
        });
    }
    let run = run_checks(ctx, args)?;
    Ok(outcome(ctx, args, &run))
}

/// A finished run.
struct Run {
    host: String,
    repo: Option<PathBuf>,
    /// Repo scope skipped: why.
    repo_skipped: Option<String>,
    reports: Vec<Report>,
}

fn run_checks(ctx: &Ctx, args: &Args) -> Result<Run> {
    let selected = select(&args.only)?;
    let host = ctx.host.trim_end_matches('/').to_owned();

    let repo =
        match &args.repo {
            Some(path) => {
                let path = ctx.env.cwd.join(path);
                Some(repo_root::find_root(&path).ok_or_else(|| {
                    Error::Usage(format!("{} is not in a git repo", path.display()))
                })?)
            }
            None => repo_root::find_root(&ctx.env.cwd),
        };
    if args.ci && repo.is_none() {
        return Err(Error::Fail(format!(
            "{} is not in a git repo; --ci checks a repo",
            ctx.env.cwd.display()
        )));
    }

    let mut repo_skipped = None;
    let defs: Vec<&Def> = CHECKS
        .iter()
        .filter(|d| selected.contains(&d.id))
        .filter(|d| match d.scope {
            Scope::User => !args.ci,
            Scope::Repo => repo.is_some(),
        })
        .collect();
    let defs: Vec<&Def> = match &repo {
        Some(root) if !args.ci && !repo::is_standards_repo(root) => {
            if defs.iter().any(|d| d.scope == Scope::Repo) {
                repo_skipped = Some(
                    "no .standards.conf and no standards submodule; run `standards init` to opt in"
                        .to_owned(),
                );
            }
            defs.into_iter()
                .filter(|d| d.scope == Scope::User)
                .collect()
        }
        _ => defs,
    };

    let mut cx = Cx {
        ctx,
        host: host.clone(),
        manifest: None,
        reach: None,
        quiet: args.quiet,
        ci: args.ci,
        repo: repo.clone(),
    };
    let needs_manifest = defs.iter().any(|d| d.needs_manifest);
    let wants_reach = defs.iter().any(|d| d.id == "mcp.reachable");
    if args.quiet {
        if needs_manifest {
            cx.manifest = quiet_manifest(ctx);
        }
    } else if needs_manifest || wants_reach {
        let (manifest, reach) = fetch_manifest(ctx)?;
        if needs_manifest && let Err(e) = manifest {
            return Err(e.into());
        }
        cx.manifest = manifest.ok();
        cx.reach = Some(reach);
    }

    let reports = defs
        .iter()
        .map(|def| check_one(&cx, def, args.fix))
        .collect();
    Ok(Run {
        host,
        repo,
        repo_skipped,
        reports,
    })
}

/// `--only` ids, validated; every check when empty.
fn select(only: &[String]) -> Result<Vec<&'static str>> {
    let ids = check_ids();
    if only.is_empty() {
        return Ok(ids);
    }
    let mut picked = Vec::new();
    for want in only.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let id = ids.iter().find(|id| **id == want).ok_or_else(|| {
            Error::Usage(format!(
                "unknown check `{want}`; one of: {}",
                ids.join(", ")
            ))
        })?;
        picked.push(*id);
    }
    Ok(picked)
}

/// The manifest with the ordinary budget, written to the cache for `--quiet`.
fn fetch_manifest(
    ctx: &Ctx,
) -> Result<(
    std::result::Result<Manifest, crate::client::ClientError>,
    std::result::Result<Duration, String>,
)> {
    let client = ctx.client_with_timeout(REACH_TIMEOUT)?;
    let started = Instant::now();
    let manifest = client.manifest();
    let reach = match &manifest {
        Ok(m) => {
            let _ = ManifestCache::for_env(&ctx.env).write(client.host(), m, SystemTime::now());
            Ok(started.elapsed())
        }
        Err(e) => Err(e.to_string()),
    };
    Ok((manifest, reach))
}

/// The cached manifest, refreshed when older than an hour with a 1s budget.
fn quiet_manifest(ctx: &Ctx) -> Option<Manifest> {
    let client = ctx.client_with_timeout(cache::QUIET_TIMEOUT).ok()?;
    ManifestCache::for_env(&ctx.env)
        .load(&client, SystemTime::now(), cache::MAX_AGE)
        .ok()
        .map(|l| l.manifest)
}

fn check_one(cx: &Cx<'_>, def: &Def, fix: bool) -> Report {
    let mut finding = (def.check)(cx);
    let mut fixed = false;
    if fix
        && finding.fixable
        && matches!(finding.level, Level::Warn | Level::Fail)
        && let Some(apply) = def.fix
    {
        fixed = true;
        finding = match apply(cx) {
            Ok(actions) if def.recheck => {
                let mut after = (def.check)(cx);
                let mut details = actions;
                details.append(&mut after.details);
                after.details = details;
                after
            }
            Ok(actions) => Finding::ok("fixed").details(actions),
            Err(e) => Finding::fail(format!("fix failed: {e}")),
        };
    }
    Report {
        id: def.id,
        scope: def.scope,
        status: finding.level,
        message: finding.message,
        fixable: finding.fixable,
        fixed,
        details: finding.details,
    }
}

fn overall(reports: &[Report]) -> Status {
    match reports.iter().map(|r| r.status).max() {
        Some(Level::Fail) => Status::Fail,
        Some(Level::Warn) => Status::Warn,
        _ => Status::Ok,
    }
}

fn data(run: &Run, args: &Args) -> serde_json::Value {
    json!({
        "host": run.host,
        "repo": run.repo.as_ref().map(|p| p.display().to_string()),
        "repo_skipped": run.repo_skipped,
        "fix": args.fix,
        "checks": run.reports,
    })
}

const ID_WIDTH: usize = 18;

fn outcome(ctx: &Ctx, args: &Args, run: &Run) -> Outcome {
    let mut out = Outcome::ok(data(run, args));
    out.status = overall(&run.reports);

    let repo_label = run
        .repo
        .as_deref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let header = |scope: Scope| match scope {
        Scope::User => "user".to_owned(),
        Scope::Repo => format!("repo {repo_label}"),
    };
    let mut last: Option<Scope> = None;
    for r in &run.reports {
        if last != Some(r.scope) {
            out.human.push(header(r.scope));
            last = Some(r.scope);
        }
        let label = if r.fixed && r.status == Level::Ok {
            "fixed"
        } else {
            r.status.label()
        };
        let hint = if r.fixable && !r.fixed && r.status > Level::Ok {
            " (fixable)"
        } else {
            ""
        };
        out.human.push(format!(
            "  {label:<5} {:<ID_WIDTH$} {}{hint}",
            r.id, r.message
        ));
        if r.status > Level::Ok || r.fixed || ctx.verbose {
            for d in &r.details {
                out.human.push(format!("          {d}"));
            }
        }
    }
    if let (Some(root), Some(why)) = (&run.repo, &run.repo_skipped) {
        out.human.push(format!("repo {}", root.display()));
        out.human.push(format!("  skip  {why}"));
    }
    out.human.push(summary(&run.reports, args.fix));
    out
}

fn summary(reports: &[Report], fix: bool) -> String {
    let count = |l: Level| reports.iter().filter(|r| r.status == l).count();
    let (fails, warns) = (count(Level::Fail), count(Level::Warn));
    let fixed = reports.iter().filter(|r| r.fixed).count();
    let fixable = reports
        .iter()
        .filter(|r| r.fixable && !r.fixed && r.status > Level::Ok)
        .count();
    let mut parts = Vec::new();
    if fixed > 0 {
        parts.push(format!("{fixed} fixed"));
    }
    if fails > 0 {
        parts.push(format!("{fails} failed"));
    }
    if warns > 0 {
        parts.push(format!(
            "{warns} {}",
            if warns == 1 { "warning" } else { "warnings" }
        ));
    }
    if parts.is_empty() || fails + warns == 0 && fixed == 0 {
        return "all checks ok".to_owned();
    }
    let mut line = parts.join(", ");
    if fixable > 0 && !fix {
        let _ = write!(line, "; `standards doctor --fix` repairs {fixable}");
    }
    line
}

/// `--quiet`: nothing when healthy, else one line naming what needs attention.
fn quiet_outcome(run: &Run) -> Outcome {
    let unhealthy: Vec<&str> = run
        .reports
        .iter()
        .filter(|r| matches!(r.status, Level::Warn | Level::Fail))
        .map(|r| r.id)
        .collect();
    let out = Outcome::ok(json!({ "checks": run.reports }));
    if unhealthy.is_empty() {
        return out;
    }
    let fix = if run
        .reports
        .iter()
        .any(|r| r.fixable && r.status > Level::Ok)
    {
        "standards doctor --fix"
    } else {
        "standards doctor"
    };
    out.line(format!(
        "standards: {} need{} attention; run `{fix}`",
        unhealthy.join(", "),
        if unhealthy.len() == 1 { "s" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(id: &'static str, status: Level, fixable: bool, fixed: bool) -> Report {
        Report {
            id,
            scope: Scope::User,
            status,
            message: String::new(),
            fixable,
            fixed,
            details: vec![],
        }
    }

    #[test]
    fn select_validates_ids() {
        assert_eq!(select(&[]).unwrap().len(), CHECKS.len());
        assert_eq!(
            select(&["repo.conf".into(), " cli.current".into(), String::new()]).unwrap(),
            vec!["repo.conf", "cli.current"]
        );
        let e = select(&["legacy".into()]).unwrap_err();
        assert!(matches!(e, Error::Usage(_)));
        assert!(e.to_string().contains("mcp.registered"));
    }

    #[test]
    fn summaries() {
        assert_eq!(summary(&[], false), "all checks ok");
        let r = [
            report("a", Level::Fail, true, false),
            report("b", Level::Warn, false, false),
            report("c", Level::Warn, false, false),
            report("d", Level::Ok, true, true),
        ];
        assert_eq!(
            summary(&r, false),
            "1 fixed, 1 failed, 2 warnings; `standards doctor --fix` repairs 1"
        );
        assert_eq!(summary(&r[1..2], true), "1 warning");
        assert_eq!(summary(&r[3..], true), "1 fixed");
        assert_eq!(overall(&r), Status::Fail);
        assert_eq!(overall(&r[1..]), Status::Warn);
        assert_eq!(overall(&r[3..]), Status::Ok);
    }

    #[test]
    fn quiet_line() {
        let run = |reports| Run {
            host: String::new(),
            repo: None,
            repo_skipped: None,
            reports,
        };
        assert_eq!(
            quiet_outcome(&run(vec![report("a", Level::Ok, false, false)])).human,
            Vec::<String>::new()
        );
        let o = quiet_outcome(&run(vec![report("a", Level::Warn, false, false)]));
        assert_eq!(
            o.human,
            ["standards: a needs attention; run `standards doctor`"]
        );
        let o = quiet_outcome(&run(vec![
            report("a", Level::Fail, true, false),
            report("b", Level::Warn, false, false),
        ]));
        assert_eq!(
            o.human,
            ["standards: a, b need attention; run `standards doctor --fix`"]
        );
        assert_eq!(o.status, Status::Ok);
    }
}
