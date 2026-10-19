//! The scan: every linked worktree of every repo, its verdict, and the plan `clean` carries
//! out. Read-only: git runs with `--no-optional-locks` where a plain call would rewrite the
//! index and so reset the age being measured.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::sys;

/// What the scan needs besides the repos.
#[derive(Debug, Clone)]
pub struct Options {
    /// A worktree untouched for fewer days is left alone.
    pub days: u64,
    /// Also report target dirs in main checkouts.
    pub main_targets: bool,
    /// `$HOME`, stripped from names.
    pub home: PathBuf,
    /// Epoch seconds.
    pub now: i64,
}

impl Options {
    fn min_age(&self) -> i64 {
        i64::try_from(self.days)
            .unwrap_or(i64::MAX / 86_400)
            .saturating_mul(86_400)
    }
}

/// One step of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// `git worktree prune` for a worktree whose directory is gone.
    Prune,
    /// `git worktree remove`.
    Remove,
    /// Delete a target dir inside a kept worktree (`--targets`).
    Targets,
    /// Delete a target dir in a main checkout (`--main-targets`).
    MainTargets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanItem {
    pub action: Action,
    pub repo: PathBuf,
    pub path: PathBuf,
    pub kib: u64,
    /// For a merged worktree's removal: the branch `--branches` deletes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// Why a worktree is kept, or how it goes; checked in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Verdict {
    /// Its directory is gone: prune the record.
    Prune,
    /// A process has its cwd inside it.
    InUse,
    Locked,
    /// Uncommitted or untracked changes (submodules included).
    Dirty {
        changes: u64,
    },
    /// No branch holds HEAD, and HEAD isn't merged.
    DetachedUnmerged,
    /// Touched more recently than `--days`.
    Recent,
    /// Removable: HEAD is merged into a default branch.
    RemoveMerged,
    /// Removable: HEAD isn't merged, but `branch` keeps the commits.
    RemoveKeptOnBranch {
        branch: String,
    },
}

impl Verdict {
    #[must_use]
    pub fn removes(&self) -> bool {
        matches!(self, Self::RemoveMerged | Self::RemoveKeptOnBranch { .. })
    }

    /// The verdict as bin/janitor words it; `age` is the worktree's age label.
    #[must_use]
    pub fn text(&self, age: &str) -> String {
        match self {
            Self::Prune => "prune (directory is gone)".to_owned(),
            Self::InUse => "keep: a process is running in it".to_owned(),
            Self::Locked => "keep: locked".to_owned(),
            Self::Dirty { changes } => format!("keep: {changes} uncommitted change(s)"),
            Self::DetachedUnmerged => "keep: detached with unmerged commits".to_owned(),
            Self::Recent => format!("keep: touched {age} ago"),
            Self::RemoveMerged => "remove (merged)".to_owned(),
            Self::RemoveKeptOnBranch { branch } => format!("remove (commits kept on {branch})"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Worktree {
    pub path: PathBuf,
    /// The path as the row shows it: relative to the repo, else to `$HOME`, cut to 34.
    pub short: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub locked: bool,
    pub verdict: Verdict,
    /// The verdict as printed, with `, target clearable` when that applies.
    pub verdict_text: String,
    /// `None` for a worktree whose directory is gone.
    #[serde(flatten)]
    pub measured: Option<Measured>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Measured {
    pub age_secs: i64,
    /// `3d`, or `5h` under a day.
    pub age: String,
    pub size_kib: u64,
    pub targets: Vec<PathBuf>,
    pub target_kib: u64,
    pub merged: bool,
    pub in_use: bool,
    pub dirty: u64,
    /// The worktree's own target dirs go in the plan (`--targets` deletes them).
    pub targets_clearable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MainTarget {
    pub path: PathBuf,
    /// The path relative to the repo.
    pub short: String,
    pub idle_days: i64,
    pub kib: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Repo {
    pub path: PathBuf,
    /// The path relative to `$HOME`.
    pub name: String,
    /// The default branches merged-ness is checked against: origin/HEAD, then main or
    /// master.
    pub defaults: Vec<String>,
    /// Linked worktrees (the main checkout is not one).
    pub worktrees: Vec<Worktree>,
    pub main_targets: Vec<MainTarget>,
}

impl Repo {
    /// Whether the repo prints at all: it has a linked worktree or a main target to show.
    #[must_use]
    pub fn shown(&self) -> bool {
        !self.worktrees.is_empty() || !self.main_targets.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub repos: Vec<Repo>,
    pub plan: Vec<PlanItem>,
    pub total_remove_kib: u64,
    pub total_targets_kib: u64,
}

/// The closing line's numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub remove: usize,
    pub remove_kib: u64,
    pub prune: usize,
    /// Target dirs in the plan, worktree and main-checkout ones together.
    pub targets: usize,
    pub targets_kib: u64,
    /// `--targets` or `--main-targets` was given, so the line says they'll be cleared.
    pub clearing_targets: bool,
}

impl Report {
    #[must_use]
    pub fn summary(&self, targets: bool, main_targets: bool) -> Summary {
        let count = |f: &dyn Fn(Action) -> bool| self.plan.iter().filter(|p| f(p.action)).count();
        Summary {
            remove: count(&|a| a == Action::Remove),
            remove_kib: self.total_remove_kib,
            prune: count(&|a| a == Action::Prune),
            targets: count(&|a| matches!(a, Action::Targets | Action::MainTargets)),
            targets_kib: self.total_targets_kib,
            clearing_targets: targets || main_targets,
        }
    }
}

/// One `worktree` stanza of `git worktree list --porcelain`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub locked: bool,
    pub prunable: bool,
}

/// Parse `git worktree list --porcelain`. The first entry is the main checkout.
#[must_use]
pub fn parse_worktrees(porcelain: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for line in porcelain.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            out.push(Entry {
                path: path.to_owned(),
                ..Entry::default()
            });
            continue;
        }
        let Some(e) = out.last_mut() else { continue };
        if let Some(head) = line.strip_prefix("HEAD ") {
            e.head = Some(head.to_owned());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            let b = branch.strip_prefix("refs/heads/").unwrap_or(branch);
            e.branch = Some(b.to_owned());
        } else if line.starts_with("locked") {
            e.locked = true;
        } else if line.starts_with("prunable") {
            e.prunable = true;
        }
    }
    out
}

/// `path` without the `prefix/` it starts with, else `None`.
fn strip_dir<'a>(path: &'a str, prefix: &Path) -> Option<&'a str> {
    path.strip_prefix(prefix.to_str()?)?.strip_prefix('/')
}

/// The row's name for a worktree: relative to the repo, else to `$HOME`, and when longer
/// than 34 characters, `…` and its last 33.
#[must_use]
pub fn short_name(wt: &str, repo: &Path, home: &Path) -> String {
    let short = strip_dir(wt, repo)
        .or_else(|| strip_dir(wt, home))
        .unwrap_or(wt);
    let n = short.chars().count();
    if n > 34 {
        let tail: String = short.chars().skip(n - 33).collect();
        format!("…{tail}")
    } else {
        short.to_owned()
    }
}

/// `3d`, or `5h` under a day.
#[must_use]
pub fn age_label(age_secs: i64) -> String {
    if age_secs < 86_400 {
        format!("{}h", age_secs / 3600)
    } else {
        format!("{}d", age_secs / 86_400)
    }
}

/// Scan every repo.
#[must_use]
pub fn scan(repos: &[PathBuf], opts: &Options, live_cwds: &[String]) -> Report {
    let mut report = Report {
        repos: Vec::new(),
        plan: Vec::new(),
        total_remove_kib: 0,
        total_targets_kib: 0,
    };
    for repo in repos {
        let r = scan_repo(repo, opts, live_cwds, &mut report);
        report.repos.push(r);
    }
    report
}

fn default_branches(repo: &Path) -> Vec<String> {
    // Merged into either the remote or the local default counts: solo repos often commit
    // to local main well before pushing.
    let mut defaults = Vec::new();
    if let Some(d) = sys::git(
        repo,
        &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"],
    ) {
        defaults.push(d.trim().to_owned());
    }
    for b in ["main", "master"] {
        if sys::git_ok(
            repo,
            &["show-ref", "-q", "--verify", &format!("refs/heads/{b}")],
        ) {
            defaults.push(b.to_owned());
            break;
        }
    }
    defaults
}

fn scan_repo(repo: &Path, opts: &Options, live_cwds: &[String], report: &mut Report) -> Repo {
    let name = strip_dir(&repo.to_string_lossy(), &opts.home)
        .map_or_else(|| repo.display().to_string(), str::to_owned);
    let defaults = default_branches(repo);
    let entries =
        parse_worktrees(&sys::git(repo, &["worktree", "list", "--porcelain"]).unwrap_or_default());
    let linked = entries.get(1..).unwrap_or_default();

    let mut worktrees = Vec::new();
    for e in linked {
        worktrees.push(scan_worktree(repo, e, &defaults, opts, live_cwds, report));
    }

    let mut main_targets = Vec::new();
    if opts.main_targets {
        for t in sys::targets_in(repo) {
            let ts = t.to_string_lossy();
            if linked
                .iter()
                .any(|w| strip_dir(&ts, Path::new(&w.path)).is_some())
            {
                continue;
            }
            let idle = opts.now - sys::newest_inner_dir(&t);
            if idle < opts.min_age() {
                continue;
            }
            let kib = sys::kib(&t);
            report.plan.push(PlanItem {
                action: Action::MainTargets,
                repo: repo.to_path_buf(),
                path: t.clone(),
                kib,
                branch: None,
            });
            report.total_targets_kib += kib;
            main_targets.push(MainTarget {
                short: strip_dir(&ts, repo).unwrap_or(&ts).to_owned(),
                path: t.clone(),
                idle_days: idle / 86_400,
                kib,
            });
        }
    }

    Repo {
        path: repo.to_path_buf(),
        name,
        defaults,
        worktrees,
        main_targets,
    }
}

fn scan_worktree(
    repo: &Path,
    e: &Entry,
    defaults: &[String],
    opts: &Options,
    live_cwds: &[String],
    report: &mut Report,
) -> Worktree {
    let wt = Path::new(&e.path);
    let mut out = Worktree {
        path: wt.to_path_buf(),
        short: short_name(&e.path, repo, &opts.home),
        head: e.head.clone(),
        branch: e.branch.clone(),
        locked: e.locked,
        verdict: Verdict::Prune,
        verdict_text: Verdict::Prune.text(""),
        measured: None,
    };
    if e.prunable || !wt.is_dir() {
        report.plan.push(PlanItem {
            action: Action::Prune,
            repo: repo.to_path_buf(),
            path: wt.to_path_buf(),
            kib: 0,
            branch: None,
        });
        return out;
    }

    let age_secs = opts.now - sys::last_touched(wt);
    let age = age_label(age_secs);
    let size_kib = sys::kib(wt);
    let targets = sys::targets_in(wt);
    let target_kib: u64 = targets.iter().map(|t| sys::kib(t)).sum();
    let dirty = sys::changes(wt);
    let merged = e.head.as_deref().is_some_and(|head| {
        defaults
            .iter()
            .any(|d| sys::git_ok(repo, &["merge-base", "--is-ancestor", head, d]))
    });
    let in_use = sys::in_use(live_cwds, &e.path);

    let verdict = if in_use {
        Verdict::InUse
    } else if e.locked {
        Verdict::Locked
    } else if dirty > 0 {
        Verdict::Dirty { changes: dirty }
    } else if !merged && e.branch.is_none() {
        Verdict::DetachedUnmerged
    } else if age_secs < opts.min_age() {
        Verdict::Recent
    } else if merged {
        Verdict::RemoveMerged
    } else {
        Verdict::RemoveKeptOnBranch {
            branch: e.branch.clone().unwrap_or_default(),
        }
    };
    let mut text = verdict.text(&age);

    let mut targets_clearable = false;
    if verdict.removes() {
        report.plan.push(PlanItem {
            action: Action::Remove,
            repo: repo.to_path_buf(),
            path: wt.to_path_buf(),
            kib: size_kib,
            branch: if merged { e.branch.clone() } else { None },
        });
        report.total_remove_kib += size_kib;
    } else if target_kib > 0 && verdict != Verdict::InUse {
        // A live build's target is never touched; any other kept worktree's is clearable.
        targets_clearable = true;
        text.push_str(", target clearable");
        for t in &targets {
            report.plan.push(PlanItem {
                action: Action::Targets,
                repo: repo.to_path_buf(),
                path: t.clone(),
                kib: sys::kib(t),
                branch: None,
            });
        }
        report.total_targets_kib += target_kib;
    }

    out.verdict = verdict;
    out.verdict_text = text;
    out.measured = Some(Measured {
        age_secs,
        age,
        size_kib,
        targets,
        target_kib,
        merged,
        in_use,
        dirty,
        targets_clearable,
    });
    out
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn porcelain_parses_every_field() {
        let p = "worktree /r\nHEAD aaa\nbranch refs/heads/main\n\n\
                 worktree /r/wt\nHEAD bbb\ndetached\nlocked reason\n\n\
                 worktree /gone\nHEAD ccc\nbranch refs/heads/x\nprunable gitdir file points to non-existent location\n";
        let e = parse_worktrees(p);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].branch.as_deref(), Some("main"));
        assert!(e[1].locked && e[1].branch.is_none() && e[1].head.as_deref() == Some("bbb"));
        assert!(e[2].prunable && !e[2].locked);
        assert_eq!(parse_worktrees("HEAD x\n"), Vec::<Entry>::new());
    }

    #[test]
    fn names_and_ages() {
        let home = Path::new("/h");
        let repo = Path::new("/h/git/r");
        assert_eq!(short_name("/h/git/r/.claude/wt", repo, home), ".claude/wt");
        assert_eq!(short_name("/h/wt/x", repo, home), "wt/x");
        assert_eq!(short_name("/elsewhere/x", repo, home), "/elsewhere/x");
        assert_eq!(short_name("/h/git/rr", repo, home), "git/rr");
        let long = format!("/h/git/r/{}", "a".repeat(40));
        assert_eq!(
            short_name(&long, repo, home),
            format!("…{}", "a".repeat(33))
        );
        assert_eq!(short_name(&long, repo, home).chars().count(), 34);
        let exact = format!("/h/git/r/{}", "b".repeat(34));
        assert_eq!(short_name(&exact, repo, home), "b".repeat(34));

        assert_eq!(age_label(0), "0h");
        assert_eq!(age_label(86_399), "23h");
        assert_eq!(age_label(86_400), "1d");
        assert_eq!(age_label(10 * 86_400 + 5), "10d");
        assert_eq!(age_label(-5), "0h");
    }
}
