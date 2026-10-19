//! Fixture trees of real repos and worktrees in a temp dir, aged by mtimes and commit
//! dates, scanned and cleaned. `bin_janitor_agrees` runs the bash script over the same tree
//! and diffs the two scans.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;

use super::clean::{self, Outcome as StepOutcome};
use super::scan::{Action, PlanItem};
use super::{Args, Mode, Term, run_with, sys};
use crate::env::{Env, EnvConfig, Platform};
use crate::output::{Io, Outcome, Status};
use crate::{Ctx, Result};

const DAY: i64 = 86_400;

/// `git` with no user or system config, a fixed identity and, given `date`, that commit
/// date. Panics on failure: the fixture must build.
fn g(dir: &Path, args: &[&str], date: Option<i64>) -> String {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args([
            "-c",
            "protocol.file.allow=always",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdin(Stdio::null());
    if let Some(d) = date {
        cmd.env("GIT_AUTHOR_DATE", format!("@{d} +0000"))
            .env("GIT_COMMITTER_DATE", format!("@{d} +0000"));
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn set_mtime(path: &Path, secs: i64) {
    let t = UNIX_EPOCH + Duration::from_secs(u64::try_from(secs).unwrap());
    File::open(path).unwrap().set_modified(t).unwrap();
}

/// A cargo-looking target dir: `CACHEDIR.TAG` when `tagged`, and some bytes.
fn make_target(dir: &Path, tagged: bool) {
    std::fs::create_dir_all(dir.join("debug/deps")).unwrap();
    if tagged {
        std::fs::write(
            dir.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
    }
    std::fs::write(dir.join("debug/deps/blob"), vec![7u8; 64 * 1024]).unwrap();
}

/// A process sitting in a directory, killed on drop.
struct Sleeper(Child);

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    proj: PathBuf,
    now: i64,
}

impl Fixture {
    fn wt(&self, name: &str) -> PathBuf {
        self.proj.join(".claude/worktrees").join(name)
    }

    fn long(&self) -> PathBuf {
        self.home
            .join("wt/a-worktree-with-a-name-long-enough-to-be-cut")
    }

    fn inuse(&self) -> String {
        self.wt("inuse").display().to_string()
    }

    fn sleeper(&self) -> Sleeper {
        Sleeper(
            Command::new("sleep")
                .arg("600")
                .current_dir(self.wt("inuse"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    fn branches(&self) -> String {
        g(&self.proj, &["branch", "--format=%(refname:short)"], None)
    }

    fn ctx(&self, json: bool) -> Ctx {
        Ctx {
            host: "http://127.0.0.1:9".into(),
            json,
            verbose: false,
            env: Env {
                home: self.home.clone(),
                xdg_cache_home: None,
                cwd: self.home.clone(),
                current_exe: None,
                version: "0.1.0".into(),
                platform: Platform::current(),
                config: EnvConfig::default(),
                path: None,
            },
        }
    }
}

/// Make a repo with one old commit on main that ignores `target/`.
fn repo(path: &Path, old: i64) {
    std::fs::create_dir_all(path).unwrap();
    g(path, &["init", "-q", "-b", "main"], None);
    std::fs::write(path.join("README.md"), "r\n").unwrap();
    std::fs::write(path.join(".gitignore"), "target/\n").unwrap();
    g(path, &["add", "-A"], None);
    g(path, &["commit", "-q", "-m", "init"], Some(old));
}

/// Push a worktree's own clocks (index, logs/HEAD) back to `at`.
fn age(wt: &Path, at: i64) {
    let gitdir = PathBuf::from(g(wt, &["rev-parse", "--absolute-git-dir"], None).trim());
    set_mtime(&gitdir.join("index"), at);
    set_mtime(&gitdir.join("logs/HEAD"), at);
}

/// The tree every test starts from (`~` is the temp dir, the default root `~/git`):
///
/// ```text
/// ~/git/proj                     main checkout, target/ idle 10d (CACHEDIR.TAG)
///   .claude/worktrees/merged     branch at main, idle 10d          remove (merged)
///   .claude/worktrees/unmerged   branch one commit ahead           remove (commits kept on unmerged)
///   .claude/worktrees/detached   detached, one commit ahead        keep: detached with unmerged commits
///   .claude/worktrees/detached-merged  detached at main            remove (merged)
///   .claude/worktrees/dirty      untracked file, tagged target     keep: 1 uncommitted change(s), target clearable
///   .claude/worktrees/faketarget untagged target dir               remove (merged), no target
///   .claude/worktrees/recent     touched now, tagged target        keep: touched 0h ago, target clearable
///   .claude/worktrees/locked     locked                            keep: locked
///   .claude/worktrees/inuse      a process's cwd, tagged target    keep: a process is running in it
///   .claude/worktrees/gone       directory deleted                 prune (directory is gone)
///   .claude/worktrees/submod     holds a submodule, branch ahead   remove (commits kept on submod)
/// ~/wt/a-worktree-with-…         proj worktree outside the repo    remove (merged), name cut
/// ~/git/other                    no worktrees                      not shown
/// ~/git/group/nested             depth 2: found, one stale worktree
/// ~/git/a/b/deep                 depth 3: below find -maxdepth 3, not found
/// ~/src/lib                      the submodule's source
/// ```
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    // Canonical, so the paths git records match the cwds lsof reports (/var → /private/var).
    let home = dir.path().canonicalize().unwrap();
    let root = home.join("git");
    let proj = root.join("proj");
    let now = sys::now();
    let old = now - 30 * DAY;
    let stale = now - 10 * DAY;
    let f = Fixture {
        _dir: dir,
        home: home.clone(),
        root: root.clone(),
        proj: proj.clone(),
        now,
    };

    repo(&proj, old);
    let add = |name: &str, extra: &[&str]| -> PathBuf {
        let path = f.wt(name);
        let p = path.to_str().unwrap().to_owned();
        let mut args = vec!["worktree", "add", "-q"];
        args.extend_from_slice(extra);
        args.push(&p);
        g(&proj, &args, None);
        path
    };
    let commit_in = |wt: &Path, file: &str| {
        std::fs::write(wt.join(file), "x\n").unwrap();
        g(wt, &["add", "-A"], None);
        g(wt, &["commit", "-q", "-m", file], Some(old));
    };

    let merged = add("merged", &["-b", "merged"]);
    age(&merged, stale);

    let unmerged = add("unmerged", &["-b", "unmerged"]);
    commit_in(&unmerged, "u.txt");
    age(&unmerged, stale);

    let detached = add("detached", &["--detach"]);
    commit_in(&detached, "d.txt");
    age(&detached, stale);

    let dm = add("detached-merged", &["--detach"]);
    age(&dm, stale);

    let dirty = add("dirty", &["-b", "dirty"]);
    std::fs::write(dirty.join("scratch.txt"), "wip\n").unwrap();
    make_target(&dirty.join("target"), true);
    age(&dirty, stale);

    let fake = add("faketarget", &["-b", "faketarget"]);
    make_target(&fake.join("target"), false);
    age(&fake, stale);

    let recent = add("recent", &["-b", "recent"]);
    make_target(&recent.join("target"), true);

    let locked = add("locked", &["-b", "locked"]);
    g(&proj, &["worktree", "lock", locked.to_str().unwrap()], None);
    age(&locked, stale);

    let inuse = add("inuse", &["-b", "inuse"]);
    make_target(&inuse.join("target"), true);
    age(&inuse, stale);

    let gone = add("gone", &["-b", "gone"]);
    std::fs::remove_dir_all(&gone).unwrap();

    let lib = home.join("src/lib");
    repo(&lib, old);
    let submod = add("submod", &["-b", "submod"]);
    g(
        &submod,
        &["submodule", "add", "-q", lib.to_str().unwrap(), "lib"],
        None,
    );
    g(&submod, &["commit", "-q", "-m", "submodule"], Some(old));
    age(&submod, stale);

    let long = f.long();
    std::fs::create_dir_all(long.parent().unwrap()).unwrap();
    g(
        &proj,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "long",
            long.to_str().unwrap(),
        ],
        None,
    );
    age(&long, stale);

    let main_target = proj.join("target");
    make_target(&main_target, true);
    set_mtime(&main_target.join("debug/deps"), stale);
    set_mtime(&main_target.join("debug"), stale);

    repo(&root.join("other"), old);

    for nested in [root.join("group/nested"), root.join("a/b/deep")] {
        repo(&nested, old);
        let wt = nested.with_file_name(format!(
            "{}-wt",
            nested.file_name().unwrap().to_str().unwrap()
        ));
        g(
            &nested,
            &["worktree", "add", "-q", "-b", "w", wt.to_str().unwrap()],
            None,
        );
        age(&wt, stale);
    }
    f
}

/// A terminal that answers `answer` when it's a terminal at all.
struct FakeTerm {
    tty: bool,
    answer: Option<&'static str>,
}

impl Term for FakeTerm {
    fn color(&self) -> bool {
        false
    }

    fn stdin_is_terminal(&self) -> bool {
        self.tty
    }

    fn read_answer(&mut self) -> Option<String> {
        self.answer.map(|a| format!("{a}\n"))
    }
}

struct Ran {
    result: Result<Outcome>,
    out: String,
    err: String,
}

fn janitor(f: &Fixture, json: bool, args: &Args, term: &mut FakeTerm, cwds: &[String]) -> Ran {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        run_with(&f.ctx(json), args, &mut io, term, cwds)
    };
    Ran {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn tty(answer: Option<&'static str>) -> FakeTerm {
    FakeTerm { tty: true, answer }
}

/// Verdict text by row name, for every shown repo.
fn verdicts(data: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for repo in data["repos"].as_array().unwrap() {
        let name = repo["name"].as_str().unwrap();
        for wt in repo["worktrees"].as_array().unwrap() {
            out.insert(
                format!("{name}:{}", wt["short"].as_str().unwrap()),
                wt["verdict_text"].as_str().unwrap().to_owned(),
            );
        }
        for t in repo["main_targets"].as_array().unwrap() {
            out.insert(
                format!("{name}:{}", t["short"].as_str().unwrap()),
                "clear (main checkout target)".to_owned(),
            );
        }
    }
    out
}

#[test]
fn scan_gives_each_worktree_its_verdict() {
    let f = fixture();
    let args = Args {
        main_targets: true,
        ..Args::default()
    };
    let r = janitor(&f, true, &args, &mut tty(None), &[f.inuse()]);
    let outcome = r.result.unwrap();
    assert_eq!(outcome.status, Status::Ok);
    let data = outcome.data;
    let expected: BTreeMap<String, String> = [
        ("git/proj:.claude/worktrees/merged", "remove (merged)"),
        (
            "git/proj:.claude/worktrees/unmerged",
            "remove (commits kept on unmerged)",
        ),
        (
            "git/proj:.claude/worktrees/detached",
            "keep: detached with unmerged commits",
        ),
        (
            "git/proj:.claude/worktrees/detached-merged",
            "remove (merged)",
        ),
        (
            "git/proj:.claude/worktrees/dirty",
            "keep: 1 uncommitted change(s), target clearable",
        ),
        ("git/proj:.claude/worktrees/faketarget", "remove (merged)"),
        (
            "git/proj:.claude/worktrees/recent",
            "keep: touched 0h ago, target clearable",
        ),
        ("git/proj:.claude/worktrees/locked", "keep: locked"),
        (
            "git/proj:.claude/worktrees/inuse",
            "keep: a process is running in it",
        ),
        (
            "git/proj:.claude/worktrees/gone",
            "prune (directory is gone)",
        ),
        (
            "git/proj:.claude/worktrees/submod",
            "remove (commits kept on submod)",
        ),
        ("git/proj:…name-long-enough-to-be-cut", "remove (merged)"),
        ("git/proj:target", "clear (main checkout target)"),
        ("git/group/nested:git/group/nested-wt", "remove (merged)"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    // The long name is cut to its last 33 characters.
    let got = verdicts(&data);
    let long_key = got.keys().find(|k| k.contains('…')).unwrap().clone();
    assert_eq!(long_key.split(':').nth(1).unwrap().chars().count(), 34);
    let got: BTreeMap<String, String> = got
        .into_iter()
        .map(|(k, v)| {
            if k == long_key {
                ("git/proj:…name-long-enough-to-be-cut".to_owned(), v)
            } else {
                (k, v)
            }
        })
        .collect();
    assert_eq!(got, expected);
    check_measurements_and_plan(&f, &data);
}

/// The rest of `scan_gives_each_worktree_its_verdict`: defaults, measurements, the plan.
fn check_measurements_and_plan(f: &Fixture, data: &Value) {
    let proj = &data["repos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "git/proj")
        .unwrap();
    assert_eq!(proj["defaults"], serde_json::json!(["main"]));
    let wts = proj["worktrees"].as_array().unwrap();
    let by = |n: &str| {
        wts.iter()
            .find(|w| w["short"] == format!(".claude/worktrees/{n}"))
            .unwrap()
    };
    assert_eq!(by("faketarget")["target_kib"], 0);
    assert!(by("dirty")["target_kib"].as_u64().unwrap() >= 64);
    assert_eq!(by("inuse")["targets_clearable"], false);
    assert_eq!(by("merged")["age"], "10d");
    assert_eq!(by("recent")["age"], "0h");
    assert_eq!(by("submod")["merged"], false);
    assert!(by("gone").get("age").is_none());
    let plan = data["plan"].as_array().unwrap();
    let count = |a: &str| plan.iter().filter(|p| p["action"] == a).count();
    assert_eq!(count("remove"), 7);
    assert_eq!(count("prune"), 1);
    assert_eq!(count("targets"), 2);
    assert_eq!(count("main-targets"), 1);
    let merged_item = plan
        .iter()
        .find(|p| p["path"] == f.wt("merged").display().to_string())
        .unwrap();
    assert_eq!(merged_item["branch"], "merged");
    let unmerged_item = plan
        .iter()
        .find(|p| p["path"] == f.wt("unmerged").display().to_string())
        .unwrap();
    assert!(unmerged_item.get("branch").is_none());
    assert_eq!(data["summary"]["remove"], 7);
    assert_eq!(data["summary"]["targets"], 3);
    assert_eq!(data["summary"]["clearing_targets"], true);
    assert_eq!(data["mode"], "scan");

    // The deep repo and the worktree-less one never show; nothing was touched.
    assert!(!got_names(data).contains(&"git/a/b/deep".to_owned()));
    assert!(f.wt("merged").is_dir());
    assert!(f.proj.join("target").is_dir());
}

fn got_names(data: &Value) -> Vec<String> {
    data["repos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn scan_prints_the_scripts_lines() {
    let f = fixture();
    let r = janitor(&f, false, &Args::default(), &mut tty(None), &[f.inuse()]);
    r.result.unwrap();
    assert_eq!(r.err, "");
    let lines: Vec<&str> = r.out.lines().collect();
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "git/group/nested");
    assert_eq!(
        lines[2],
        format!(
            "  {:<34} {:>5} {:>7} {:>7}  verdict",
            "worktree", "age", "size", "target"
        )
    );
    assert!(lines[3].starts_with(&format!("  {:<34}   10d ", "git/group/nested-wt")));
    assert!(lines[3].ends_with("  remove (merged)"));
    assert!(r.out.contains("\ngit/proj\n"));
    assert!(r.out.contains(&format!(
        "  {:<34}     -       -       -  prune (directory is gone)\n",
        ".claude/worktrees/gone"
    )));
    // No --main-targets: the main checkout's target isn't a row, and target dirs of kept
    // worktrees are only mentioned.
    assert!(!r.out.contains("main checkout target"));
    let n = lines.len();
    assert_eq!(lines[n - 3], "");
    assert!(
        lines[n - 2].starts_with("7 worktree(s) to remove ("),
        "{}",
        lines[n - 2]
    );
    assert!(lines[n - 2].contains("), 1 gone to prune, plus "));
    assert!(lines[n - 2].ends_with(" in target dirs of kept worktrees (--targets)"));
    assert_eq!(
        lines[n - 1],
        "run `standards janitor clean` (same options) to do it"
    );
}

#[test]
fn clean_yes_removes_clears_and_prunes() {
    let f = fixture();
    let args = Args {
        mode: Some(Mode::Clean),
        targets: true,
        main_targets: true,
        branches: true,
        yes: true,
        ..Args::default()
    };
    let r = janitor(&f, false, &args, &mut tty(None), &[f.inuse()]);
    assert_eq!(r.result.unwrap().status, Status::Ok);

    for gone in [
        "merged",
        "unmerged",
        "detached-merged",
        "faketarget",
        "submod",
    ] {
        assert!(!f.wt(gone).exists(), "{gone} still there");
        assert!(
            r.out
                .contains(&format!("removed {}\n", f.wt(gone).display()))
        );
    }
    assert!(!f.long().exists());
    for kept in ["detached", "dirty", "recent", "locked", "inuse"] {
        assert!(f.wt(kept).is_dir(), "{kept} removed");
    }
    // Targets of kept worktrees are cleared, except the live one's.
    assert!(!f.wt("dirty").join("target").exists());
    assert!(!f.wt("recent").join("target").exists());
    assert!(f.wt("inuse").join("target").is_dir());
    assert!(!f.proj.join("target").exists());
    assert!(
        r.out
            .contains(&format!("cleared {}\n", f.proj.join("target").display()))
    );

    // --branches deletes merged branches only; unmerged ones keep their commits.
    let branches = f.branches();
    for b in ["merged", "faketarget", "long"] {
        assert!(!branches.lines().any(|l| l == b), "{b} not deleted");
        assert!(r.out.contains(&format!("        deleted branch {b}\n")));
    }
    for b in ["unmerged", "submod", "dirty", "gone"] {
        assert!(branches.lines().any(|l| l == b), "{b} deleted");
    }

    // The gone worktree was pruned.
    let list = g(&f.proj, &["worktree", "list", "--porcelain"], None);
    assert!(!list.contains("worktrees/gone"));
    assert!(r.out.contains("\nfreed about "));

    // The deep repo, never found, is untouched.
    assert!(f.root.join("a/b/deep-wt").is_dir());
}

#[test]
fn clean_asks_first() {
    let f = fixture();
    let args = Args {
        mode: Some(Mode::Clean),
        ..Args::default()
    };

    // Not a terminal: refuse without touching anything.
    let r = janitor(
        &f,
        false,
        &args,
        &mut FakeTerm {
            tty: false,
            answer: None,
        },
        &[],
    );
    let e = r.result.unwrap_err();
    assert_eq!(
        e.to_string(),
        "not a terminal; pass --yes to clean without a prompt"
    );
    assert_eq!(e.exit_code(), 1);
    assert!(f.wt("merged").is_dir());

    // --json can't prompt either.
    let r = janitor(&f, true, &args, &mut tty(Some("y")), &[]);
    assert!(r.result.unwrap_err().to_string().contains("pass --yes"));
    assert_eq!(r.out, "");

    // Declined (and end of input): nothing changes.
    for answer in [Some("n"), None] {
        let r = janitor(&f, false, &args, &mut tty(answer), &[]);
        assert_eq!(r.result.unwrap().data["clean"]["reason"], "declined");
        assert!(
            r.out.ends_with("go ahead? [y/N] nothing changed\n"),
            "{}",
            r.out
        );
        assert!(f.wt("merged").is_dir());
    }

    // Yes, without --targets: worktrees go, target dirs of kept worktrees stay. Nothing is
    // in use this time, so inuse goes too.
    let r = janitor(&f, false, &args, &mut tty(Some(" yes ")), &[]);
    let data = r.result.unwrap().data;
    assert!(!f.wt("merged").exists());
    assert!(f.wt("dirty").join("target").is_dir());
    assert!(f.proj.join("target").is_dir());
    assert!(f.branches().lines().any(|l| l == "merged"));
    assert!(
        r.out
            .contains(&format!("removed {}\n", f.wt("merged").display()))
    );
    assert!(!r.out.contains("cleared"));
    let results = data["clean"]["results"].as_array().unwrap();
    assert!(results.iter().all(|s| s["action"] != "targets"));

    // Again: the stale worktrees are gone; what's left is only clearable targets, which
    // without --targets means nothing to do.
    let r = janitor(&f, false, &args, &mut tty(Some("y")), &[]);
    let out = r.result.unwrap();
    assert_eq!(out.data["clean"]["reason"], "nothing to do");
    assert!(r.out.ends_with("nothing to do\n"), "{}", r.out);
}

#[test]
fn clean_in_json_reports_each_step() {
    let f = fixture();
    let args = Args {
        mode: Some(Mode::Clean),
        yes: true,
        roots: vec![PathBuf::from("git/group")],
        ..Args::default()
    };
    let r = janitor(&f, true, &args, &mut tty(None), &[]);
    let data = r.result.unwrap().data;
    assert_eq!(r.out, "", "json mode prints nothing itself");
    let results = data["clean"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["result"], "removed");
    assert_eq!(results[0]["deleted_branch"], Value::Null);
    assert!(data["clean"]["freed_kib"].as_u64().unwrap() > 0);
    assert_eq!(data["roots"][0], f.root.join("group").display().to_string());
}

#[test]
fn clean_skips_what_changed_or_git_refuses() {
    let f = fixture();
    // A submodule worktree that went dirty after the scan, a dirty one git won't remove,
    // a "target" that's a file, and a target already gone.
    std::fs::write(f.wt("submod").join("new.txt"), "x").unwrap();
    std::fs::write(f.home.join("not-a-dir"), "x").unwrap();
    let item = |action, path: PathBuf| PlanItem {
        action,
        repo: f.proj.clone(),
        path,
        kib: 10,
        branch: Some("submod".into()),
    };
    let plan = vec![
        item(Action::Remove, f.wt("submod")),
        item(Action::Remove, f.wt("dirty")),
        item(Action::Targets, f.home.join("not-a-dir")),
        item(Action::MainTargets, f.home.join("never-was")),
        item(Action::Prune, f.wt("gone")),
    ];
    let r = clean::execute(&plan, true);
    let outcomes: Vec<&StepOutcome> = r.results.iter().map(|s| &s.outcome).collect();
    assert_eq!(
        outcomes,
        vec![
            &StepOutcome::SkippedChanged,
            &StepOutcome::SkippedRefused,
            &StepOutcome::Failed,
            &StepOutcome::Cleared,
            &StepOutcome::Pruned,
        ]
    );
    assert!(r.skipped());
    assert_eq!(r.freed_kib, 10);
    assert!(f.wt("submod").is_dir() && f.wt("dirty").is_dir());

    let mut out = Vec::new();
    super::render::clean(&mut out, &r, &super::render::Palette::new(false));
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains(&format!(
        "skipped {} (changed since the scan)\n",
        f.wt("submod").display()
    )));
    assert!(out.contains(&format!(
        "skipped {} (git refused, left as is)\n",
        f.wt("dirty").display()
    )));
    assert!(out.ends_with("\nfreed about 10K\n"));

    // Through the command, a refusal makes the run a warning (still exit 0).
    std::fs::write(f.wt("merged").join("late.txt"), "x").unwrap();
    let args = Args {
        mode: Some(Mode::Clean),
        yes: true,
        ..Args::default()
    };
    let r = janitor(&f, false, &args, &mut tty(None), &[]);
    // The scan sees merged as dirty now, so it's kept; nothing refused, status ok.
    assert_eq!(r.result.unwrap().status, Status::Ok);
}

#[test]
fn roots_missing_or_empty() {
    let f = fixture();
    let args = Args {
        roots: vec![f.home.join("nope"), f.home.join("src/lib/.git/objects")],
        ..Args::default()
    };
    let r = janitor(&f, false, &args, &mut tty(None), &[]);
    let e = r.result.unwrap_err();
    assert_eq!(
        e.to_string(),
        format!(
            "no git repos under {} {}",
            f.home.join("nope").display(),
            f.home.join("src/lib/.git/objects").display()
        )
    );
    assert_eq!(
        r.err,
        format!("warning: no such root {}\n", f.home.join("nope").display())
    );

    // A root with repos but nothing to report prints just the summary.
    let args = Args {
        roots: vec![f.home.join("src")],
        ..Args::default()
    };
    let r = janitor(&f, false, &args, &mut tty(None), &[]);
    r.result.unwrap();
    assert_eq!(
        r.out,
        "\n0 worktree(s) to remove (0K), 0 gone to prune\nrun `standards janitor clean` (same options) to do it\n"
    );
    assert!(f.now > 0);
}

#[test]
fn the_cli_surface() {
    let f = fixture();
    let run = |args: &[&str]| {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = {
            let mut io = Io {
                out: &mut out,
                err: &mut err,
            };
            crate::run(
                std::iter::once("standards").chain(args.iter().copied()),
                f.ctx(false).env,
                &mut io,
            )
        };
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    };
    let root = f.root.join("group").display().to_string();
    let (code, out, _) = run(&["--json", "janitor", "--root", &root, "--days", "30"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["data"]["days"], 30);
    assert_eq!(
        v["data"]["repos"][0]["worktrees"][0]["verdict_text"],
        "keep: touched 10d ago"
    );
    let (code, out, _) = run(&["--json", "janitor", "clean", "--root", &root]);
    assert_eq!(code, 1);
    assert!(out.contains("pass --yes"));
    for bad in [
        &["janitor", "--days", "x"][..],
        &["janitor", "--bogus"],
        &["janitor", "wipe"],
    ] {
        assert_eq!(run(bad).0, 2, "{bad:?}");
    }
    let (code, out, _) = run(&["janitor", "--help"]);
    assert_eq!(code, 0);
    for flag in [
        "--root",
        "--days",
        "--targets",
        "--main-targets",
        "--branches",
        "--yes",
    ] {
        assert!(out.contains(flag), "{flag}");
    }
}

/// The bash script and the port, over the same tree, print the same scan. Skipped (passes)
/// when bash or lsof is missing.
#[test]
fn bin_janitor_agrees() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/janitor");
    let has = |tool: &str| {
        Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if !script.is_file() || !has("bash") || !has("lsof") {
        return;
    }
    let f = fixture();
    // bin/janitor dies silently (set -e) when the last `target` dir it finds in a worktree
    // has no CACHEDIR.TAG: `targets_in`'s loop ends on a failed test. The port reads that as
    // "no target dir" (`scan_gives_each_worktree_its_verdict` checks it); here the untagged
    // one goes so the script can run.
    std::fs::remove_dir_all(f.wt("faketarget").join("target")).unwrap();
    let _sleeper = f.sleeper();
    let cwds = sys::live_cwds();
    assert!(sys::in_use(&cwds, &f.inuse()), "lsof can't see the sleeper");

    for flags in [&[][..], &["--main-targets"], &["--targets", "--days", "20"]] {
        let bash = Command::new("bash")
            .arg(&script)
            .arg("scan")
            .args(flags)
            .env("HOME", &f.home)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            bash.status.success(),
            "{}: {}{}",
            bash.status,
            String::from_utf8_lossy(&bash.stderr),
            String::from_utf8_lossy(&bash.stdout)
        );
        let bash = String::from_utf8(bash.stdout).unwrap();

        let mut args = Args::default();
        for flag in flags {
            match *flag {
                "--main-targets" => args.main_targets = true,
                "--targets" => args.targets = true,
                "--days" => args.days = 20,
                _ => {}
            }
        }
        let ours = janitor(&f, false, &args, &mut tty(None), &cwds);
        ours.result.unwrap();

        // The last line names the command to run next; everything else is identical.
        let strip = |s: &str| s.lines().collect::<Vec<_>>()[..s.lines().count() - 1].join("\n");
        assert_eq!(strip(&ours.out), strip(&bash), "flags {flags:?}");
        assert!(bash.ends_with("run `janitor clean` (same options) to do it\n"));
        assert!(bash.contains("keep: a process is running in it"));
    }
}
