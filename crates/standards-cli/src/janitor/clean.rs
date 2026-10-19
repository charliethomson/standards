//! Carrying out the plan: remove worktrees, clear target dirs, then `git worktree prune`
//! once per repo the plan touched.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;

use super::scan::{Action, PlanItem};
use super::sys;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum Outcome {
    /// `git worktree remove` succeeded; with `--branches`, the merged branch it held is gone
    /// too when `git branch -d` agreed.
    Removed { deleted_branch: Option<String> },
    /// It holds submodules and is no longer clean, so it wasn't forced.
    SkippedChanged,
    /// git would not remove it.
    SkippedRefused,
    /// A target dir was deleted.
    Cleared,
    /// A target dir could not be deleted.
    Failed,
    /// A gone worktree's record, pruned with its repo.
    Pruned,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub action: Action,
    pub path: PathBuf,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanResult {
    pub results: Vec<Step>,
    pub freed_kib: u64,
}

impl CleanResult {
    /// Something in the plan was left as is.
    #[must_use]
    pub fn skipped(&self) -> bool {
        self.results.iter().any(|s| {
            matches!(
                s.outcome,
                Outcome::SkippedChanged | Outcome::SkippedRefused | Outcome::Failed
            )
        })
    }
}

fn remove(item: &PlanItem, branches: bool) -> Outcome {
    let path = item.path.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    // git refuses any worktree containing submodules without --force. The scan already
    // checked it's clean (submodules included), so re-check right before forcing.
    if item.path.join(".gitmodules").is_file() {
        if sys::changes(&item.path) > 0 {
            return Outcome::SkippedChanged;
        }
        args.push("--force");
    }
    args.push(&path);
    if !sys::git_ok(&item.repo, &args) {
        return Outcome::SkippedRefused;
    }
    let deleted_branch = match (&item.branch, branches) {
        (Some(b), true) if sys::git_ok(&item.repo, &["branch", "-d", b]) => Some(b.clone()),
        _ => None,
    };
    Outcome::Removed { deleted_branch }
}

/// Do every step of `plan` in order.
#[must_use]
pub fn execute(plan: &[PlanItem], branches: bool) -> CleanResult {
    let mut results = Vec::new();
    let mut freed_kib = 0;
    for item in plan {
        let outcome = match item.action {
            Action::Prune => Outcome::Pruned,
            Action::Remove => remove(item, branches),
            Action::Targets | Action::MainTargets => {
                // `rm -rf`: a path already gone counts as cleared.
                match std::fs::remove_dir_all(&item.path) {
                    Ok(()) => Outcome::Cleared,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Outcome::Cleared,
                    Err(_) => Outcome::Failed,
                }
            }
        };
        if matches!(outcome, Outcome::Removed { .. } | Outcome::Cleared) {
            freed_kib += item.kib;
        }
        results.push(Step {
            action: item.action,
            path: item.path.clone(),
            outcome,
        });
    }
    let repos: BTreeSet<&PathBuf> = plan.iter().map(|p| &p.repo).collect();
    for repo in repos {
        sys::git_ok(repo, &["worktree", "prune"]);
    }
    CleanResult { results, freed_kib }
}
