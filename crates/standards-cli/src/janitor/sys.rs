//! The machine, as bin/janitor saw it: `git`, `lsof` and `du` are shelled out to, as the
//! script did (L16); the `find` and `stat` calls are walked in-process with the same rules.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the epoch.
#[must_use]
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `git -C <dir> <args>`: stdout when it exits 0, stderr discarded.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `git -C <dir> <args>`, true when it exits 0. Output discarded.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// When a worktree was last touched, in epoch seconds: the newest of its index mtime, its
/// `logs/HEAD` mtime and its last commit.
#[must_use]
pub fn last_touched(wt: &Path) -> i64 {
    let gitdir = git(wt, &["rev-parse", "--absolute-git-dir"])
        .map_or_else(|| wt.join(".git"), |s| PathBuf::from(s.trim()));
    let last_commit = git(wt, &["log", "-1", "--format=%ct"])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    mtime(&gitdir.join("index"))
        .max(mtime(&gitdir.join("logs/HEAD")))
        .max(last_commit)
}

/// Uncommitted and untracked changes in a worktree, submodules included; 0 when git fails.
/// `--no-optional-locks`: a plain status rewrites the index, which would reset the age
/// [`last_touched`] reads.
#[must_use]
pub fn changes(wt: &Path) -> u64 {
    git(
        wt,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=normal",
        ],
    )
    .map_or(0, |s| s.lines().count() as u64)
}

/// Every process's cwd, once (`lsof -a -d cwd -Fn`). A worktree with a live shell, agent or
/// dev server in it is in use. Empty when lsof is missing or fails.
#[must_use]
pub fn live_cwds() -> Vec<String> {
    let Ok(out) = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-Fn"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    parse_lsof(&String::from_utf8_lossy(&out.stdout))
}

/// The `n` lines of `lsof -F` output, sorted and unique.
#[must_use]
pub fn parse_lsof(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| l.strip_prefix('n'))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// A process has its cwd at or under `path`.
#[must_use]
pub fn in_use(live_cwds: &[String], path: &str) -> bool {
    live_cwds.iter().any(|c| {
        c.strip_prefix(path)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

/// `du -sk`: the KiB a path holds on disk, 0 when du fails.
#[must_use]
pub fn kib(path: &Path) -> u64 {
    Command::new("du")
        .arg("-sk")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

/// A path's mtime in epoch seconds, 0 when it can't be read.
#[must_use]
pub fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A real directory (not a symlink to one), as `find -type d` sees it.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// The subdirectories of `dir`, sorted by name.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_real_dir(p))
        .collect();
    out.sort();
    out
}

fn name_is(path: &Path, name: &str) -> bool {
    path.file_name().is_some_and(|n| n == name)
}

/// Repos under the roots: `find <root> -maxdepth 3 -type d -name .git -prune`, minus the
/// `/.git`, sorted and unique. A root that isn't a directory is skipped.
#[must_use]
pub fn find_repos(roots: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut BTreeSet<PathBuf>) {
        for sub in subdirs(dir) {
            if name_is(&sub, ".git") {
                out.insert(dir.to_path_buf());
            } else if depth + 1 < 3 {
                walk(&sub, depth + 1, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    for root in roots.iter().filter(|r| r.is_dir()) {
        walk(root, 0, &mut out);
    }
    out.into_iter().collect()
}

/// Cargo target dirs under a path: `find <dir> -maxdepth 5`, pruning `.git` and
/// `node_modules`, taking (and not descending into) each dir called `target` that holds a
/// `CACHEDIR.TAG` (which marks a real one, not a dir that happens to be called target).
#[must_use]
pub fn targets_in(dir: &Path) -> Vec<PathBuf> {
    fn visit(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if name_is(dir, ".git") || name_is(dir, "node_modules") {
            return;
        }
        if name_is(dir, "target") {
            if dir.join("CACHEDIR.TAG").is_file() {
                out.push(dir.to_path_buf());
            }
            return;
        }
        if depth < 5 {
            for sub in subdirs(dir) {
                visit(&sub, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    if is_real_dir(dir) {
        visit(dir, 0, &mut out);
    }
    out
}

/// The newest mtime of the directories one or two levels inside `dir`, 0 when none:
/// how recently a build wrote to a target dir.
#[must_use]
pub fn newest_inner_dir(dir: &Path) -> i64 {
    subdirs(dir)
        .into_iter()
        .flat_map(|d| {
            let inner = subdirs(&d);
            std::iter::once(d).chain(inner)
        })
        .map(|d| mtime(&d))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_lines_and_prefix_matching() {
        let cwds = parse_lsof("p12\nfcwd\nn/a/b\np13\nfcwd\nn/a/b\nn/c\n");
        assert_eq!(cwds, vec!["/a/b".to_owned(), "/c".to_owned()]);
        assert!(in_use(&cwds, "/a/b"));
        assert!(in_use(&cwds, "/a"));
        assert!(!in_use(&cwds, "/a/bc"));
        assert!(!in_use(&cwds, "/a/b/c"));
        assert!(!in_use(&[], "/a"));
    }

    #[test]
    fn missing_paths_read_as_zero() {
        let p = Path::new("/nonexistent/janitor/path");
        assert_eq!(mtime(p), 0);
        assert_eq!(kib(p), 0);
        assert_eq!(newest_inner_dir(p), 0);
        assert_eq!(targets_in(p), Vec::<PathBuf>::new());
        assert!(git(p, &["status"]).is_none());
        assert!(!git_ok(p, &["status"]));
        assert!(now() > 1_700_000_000);
    }

    #[test]
    fn discovery_depths_match_find() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        for d in [
            "a/.git",
            "g/b/.git",
            "g/h/too-deep/.git",
            "c/.git/modules/x/.git",
            "n/node_modules/x/target",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let repos = find_repos(&[root.to_path_buf(), root.join("missing")]);
        assert_eq!(
            repos,
            vec![root.join("a"), root.join("c"), root.join("g/b")]
        );

        for d in [
            "r/target/debug",
            "r/sub/target",
            "r/plain/target",
            "r/.git/target",
            "r/node_modules/target",
            "r/1/2/3/4/target",
            "r/1/2/3/4/5/target",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for d in [
            "r/target",
            "r/sub/target",
            "r/.git/target",
            "r/node_modules/target",
            "r/1/2/3/4/target",
            "r/1/2/3/4/5/target",
        ] {
            std::fs::write(root.join(d).join("CACHEDIR.TAG"), "Signature").unwrap();
        }
        let found = targets_in(&root.join("r"));
        assert_eq!(
            found,
            vec![
                root.join("r/1/2/3/4/target"),
                root.join("r/sub/target"),
                root.join("r/target"),
            ]
        );
        assert!(newest_inner_dir(&root.join("r/target")) > 0);
        assert_eq!(newest_inner_dir(&root.join("r/sub/target")), 0);
    }
}
