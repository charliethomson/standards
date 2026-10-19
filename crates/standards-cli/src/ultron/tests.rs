//! Golden tests against `bin/ultron`, plus the behaviour the goldens can't reach (watch on a
//! terminal, gc, log, --json, colour).
//!
//! The fixtures are four real programs copied from `~/.local/state/thmsn/ultron/`
//! (`tests/fixtures/ultron/<slug>/`), their real mtimes (`mtimes.tsv`) and the capture time
//! (`now`). The goldens (`golden/*.txt`) are `bin/ultron`'s output over a scratch copy of
//! the same files with `date +%s` pinned to `now`, paths normalised to `@STATE@`. Each test
//! rebuilds that scratch state dir and runs the port with the same clock.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use super::host::{Host, RealHost};
use super::state::{StateDir, fingerprint};
use super::view::Style;
use super::watch::{WatchArgs, Watcher};
use super::*;
use crate::env::{Env, EnvConfig, Platform};
use crate::output::Io;

const PROGRAMS: [&str; 4] = [
    "standards-hosted",
    "fleet-backups",
    "stash",
    "tav-backlog-audit",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ultron")
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// A scratch `XDG_STATE_HOME` holding the fixture programs with their recorded mtimes.
struct World {
    tmp: tempfile::TempDir,
    now: i64,
}

impl World {
    fn empty() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            now: RealHost.now(),
        }
    }

    fn fixtures() -> Self {
        let w = Self::empty();
        let fx = fixtures();
        for p in PROGRAMS {
            copy_tree(&fx.join(p), &w.ultron().join(p));
        }
        for line in fs::read_to_string(fx.join("mtimes.tsv")).unwrap().lines() {
            let (rel, m) = line.split_once('\t').unwrap();
            super::state::tests::set_mtime(
                w.ultron().join(rel).to_str().unwrap(),
                m.parse().unwrap(),
            );
        }
        let now = fs::read_to_string(fx.join("now")).unwrap();
        Self {
            now: now.trim().parse().unwrap(),
            ..w
        }
    }

    fn state_home(&self) -> PathBuf {
        self.tmp.path().join("state")
    }

    fn ultron(&self) -> PathBuf {
        self.state_home().join("thmsn/ultron")
    }

    fn ultron_str(&self) -> String {
        self.ultron().to_str().unwrap().to_owned()
    }

    fn ctx(&self, json: bool, tty: bool) -> Ctx {
        Ctx {
            host: String::new(),
            json,
            verbose: false,
            env: Env {
                home: self.tmp.path().join("home"),
                xdg_cache_home: None,
                cwd: self.tmp.path().to_owned(),
                current_exe: None,
                version: "0.1.0".into(),
                platform: Platform::current(),
                config: EnvConfig::default(),
                path: None,
                xdg_state_home: Some(self.state_home()),
                stdout_tty: tty,
            },
        }
    }

    fn golden(&self, name: &str) -> String {
        fs::read_to_string(fixtures().join("golden").join(name))
            .unwrap()
            .replace("@STATE@", self.state_home().to_str().unwrap())
    }
}

/// A scripted process: a fixed clock, canned stdin, and naps that run an action each and
/// stop the watch when they run out.
struct FakeHost {
    now: i64,
    input: VecDeque<String>,
    naps: VecDeque<Box<dyn FnMut()>>,
    trapped: bool,
}

impl FakeHost {
    fn new(now: i64) -> Self {
        Self {
            now,
            input: VecDeque::new(),
            naps: VecDeque::new(),
            trapped: false,
        }
    }
}

impl Host for FakeHost {
    fn now(&self) -> i64 {
        self.now
    }
    fn clock(&self) -> String {
        "12:34:56".into()
    }
    fn read_line(&mut self) -> Option<String> {
        self.input.pop_front()
    }
    fn trap_signals(&mut self) {
        self.trapped = true;
    }
    fn nap(&mut self, _secs: u64) -> bool {
        match self.naps.pop_front() {
            Some(mut f) => {
                f();
                true
            }
            None => false,
        }
    }
}

struct Ran {
    result: Result<Outcome>,
    out: String,
    err: String,
}

fn run_in(ctx: &Ctx, host: &mut FakeHost, args: &[&str]) -> Ran {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        let args = Args {
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        };
        run_with(ctx, &args, &mut io, host)
    };
    Ran {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn run_text(w: &World, args: &[&str]) -> Ran {
    run_in(&w.ctx(false, false), &mut FakeHost::new(w.now), args)
}

fn data(r: &Ran) -> serde_json::Value {
    r.result.as_ref().unwrap().data.clone()
}

fn err(r: &Ran) -> String {
    r.result.as_ref().unwrap_err().to_string()
}

// ── goldens ───────────────────────────────────────────────────────────────────

#[test]
fn ls_matches_the_script() {
    let w = World::fixtures();
    for args in [&[][..], &["ls"], &["list"], &[""]] {
        let r = run_text(&w, args);
        assert!(r.result.is_ok());
        assert_eq!(r.out, w.golden("ls.txt"), "{args:?}");
    }
}

#[test]
fn status_matches_the_script_for_every_fixture() {
    let w = World::fixtures();
    for p in PROGRAMS {
        for cmd in ["status", "st"] {
            let r = run_text(&w, &[cmd, p]);
            assert!(r.result.is_ok(), "{p}: {}", err(&r));
            assert_eq!(r.out, w.golden(&format!("status-{p}.txt")), "{p}");
        }
    }
    // No argument: the most recently touched program.
    let r = run_text(&w, &["status"]);
    assert_eq!(r.out, w.golden("status.txt"));
    let r = run_text(&w, &["status", ""]);
    assert_eq!(r.out, w.golden("status.txt"));
}

#[test]
fn help_and_log_match_the_script() {
    let w = World::fixtures();
    for flag in ["--help", "-h", "help"] {
        assert_eq!(run_text(&w, &[flag]).out, w.golden("help.txt"));
    }
    for cmd in ["log", "cat"] {
        let r = run_text(&w, &[cmd, "STASH-9"]);
        assert_eq!(r.out, w.golden("log-STASH-9.txt"));
    }
}

#[test]
fn watch_off_a_terminal_prints_status_once() {
    let w = World::fixtures();
    let mut host = FakeHost::new(w.now);
    let r = run_in(
        &w.ctx(false, false),
        &mut host,
        &["watch", "-n", "5", "stash"],
    );
    assert!(r.result.is_ok());
    assert_eq!(r.out, w.golden("status-stash.txt"));
    assert!(!host.trapped, "never took over the screen");
    let r = run_text(&w, &["w"]);
    assert_eq!(r.out, w.golden("status.txt"));
}

// ── errors (the script's `die`: exit 1, one line on stderr) ─────────────────────

#[test]
fn bad_arguments_are_the_scripts_errors() {
    let w = World::fixtures();
    let cases: [(&[&str], &str); 10] = [
        (&["bogus"], "unknown command: bogus (try 'ultron --help')"),
        (
            &["status", "nope"],
            "no such program: nope (try 'ultron ls')",
        ),
        (&["log"], "usage: ultron log <TASK-ID>"),
        (&["log", "NOPE-1"], "no journal for NOPE-1"),
        (
            &["gc"],
            "usage: ultron gc <program>   (name it explicitly \u{2014} this deletes state)",
        ),
        (&["gc", "nope"], "no such program: nope (try 'ultron ls')"),
        (&["watch", "-n"], "--interval needs a value"),
        (
            &["watch", "-n", "x"],
            "--interval must be a whole number of seconds",
        ),
        (
            &["watch", "--interval", "0"],
            "--interval must be at least 1 second",
        ),
        (&["watch", "--bogus"], "unknown option: --bogus"),
    ];
    for (args, msg) in cases {
        let r = run_text(&w, args);
        let e = r.result.as_ref().unwrap_err();
        assert_eq!(e.to_string(), msg, "{args:?}");
        assert_eq!(e.exit_code(), crate::exit::FAIL);
        assert_eq!(r.out, "");
    }
    let r = run_text(&w, &["watch", "nope"]);
    assert_eq!(err(&r), "no such program: nope (try 'ultron ls')");
}

#[test]
fn no_programs() {
    let w = World::empty();
    let r = run_text(&w, &["ls"]);
    assert_eq!(r.out, format!("no programs under {}\n", w.ultron_str()));
    let r = run_text(&w, &["status"]);
    assert_eq!(err(&r), format!("no programs under {}", w.ultron_str()));
}

// ── watch on a terminal ─────────────────────────────────────────────────────────

#[test]
fn watch_args() {
    assert_eq!(
        WatchArgs::parse(&[]).unwrap(),
        WatchArgs {
            interval: 2,
            slug: None
        }
    );
    assert_eq!(
        WatchArgs::parse(&["a", "--interval", "007", "b"]).unwrap(),
        WatchArgs {
            interval: 7,
            slug: Some("b".into())
        }
    );
    assert!(WatchArgs::parse(&["-n", ""]).is_err());
    assert!(WatchArgs::parse(&["-n", "99999999999999999999999"]).is_err());
    assert!(WatchArgs::parse(&["-"]).is_err());
}

#[test]
fn watch_redraws_only_when_the_fingerprint_changes() {
    let w = World::fixtures();
    let state = StateDir::new(w.ultron_str());
    let host = FakeHost::new(w.now);
    let args = WatchArgs::parse(&["stash", "-n", "3"]).unwrap();
    let mut watcher = Watcher::new(&state, args, Style::new(false));
    let mut out = Vec::new();

    assert!(watcher.tick(&mut out, &host).unwrap());
    let first = String::from_utf8(out.clone()).unwrap();
    assert!(first.starts_with("\x1b[H\x1b[2J"));
    let body = first.trim_start_matches("\x1b[H\x1b[2J");
    assert_eq!(
        body,
        format!(
            "{}\nlast change 12:34:56 \u{b7} polling every 3s \u{b7} ctrl-c to stop\n",
            w.golden("status-stash.txt")
        )
    );

    // Nothing changed: no redraw.
    out.clear();
    assert!(!watcher.tick(&mut out, &host).unwrap());
    assert_eq!(out, b"");

    // A journal touched: redraw.
    let j = w.ultron().join("stash/STASH-9.md");
    super::state::tests::set_mtime(j.to_str().unwrap(), 1);
    assert!(watcher.tick(&mut out, &host).unwrap());

    // The program deleted under the watch: the script's error.
    fs::remove_dir_all(w.ultron().join("stash")).unwrap();
    let e = watcher.tick(&mut out, &host).unwrap_err();
    assert_eq!(e.to_string(), "no such program: stash (try 'ultron ls')");
}

#[test]
fn watch_waits_for_a_program_then_follows_the_newest() {
    let w = World::empty();
    let state = StateDir::new(w.ultron_str());
    let host = FakeHost::new(w.now);
    let mut watcher = Watcher::new(&state, WatchArgs::parse(&[]).unwrap(), Style::new(false));
    let mut out = Vec::new();
    assert!(watcher.tick(&mut out, &host).unwrap());
    assert_eq!(
        String::from_utf8(out.clone()).unwrap(),
        format!(
            "\x1b[H\x1b[2Jwaiting for a program under {} \u{2026}\n",
            w.ultron_str()
        )
    );
    out.clear();
    assert!(!watcher.tick(&mut out, &host).unwrap(), "waits quietly");

    // An empty program directory still draws once.
    fs::create_dir_all(w.ultron().join("fresh")).unwrap();
    assert!(watcher.tick(&mut out, &host).unwrap());
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("fresh  "), "{s}");
    assert!(s.contains("! no PROGRAM.md"), "{s}");
}

#[test]
fn watch_takes_the_screen_and_gives_it_back() {
    let w = World::fixtures();
    let ctx = w.ctx(false, true);
    let mut host = FakeHost::new(w.now);
    let journal = w.ultron().join("stash/STASH-9.md");
    host.naps.push_back(Box::new(|| {})); // quiet poll
    host.naps.push_back(Box::new(move || {
        super::state::tests::set_mtime(journal.to_str().unwrap(), 2);
    }));
    let r = run_in(&ctx, &mut host, &["watch", "stash"]);
    assert!(r.result.is_ok());
    assert!(host.trapped);
    assert!(r.out.starts_with("\x1b[?1049h\x1b[?25l\x1b[H\x1b[2J"));
    assert!(r.out.ends_with("\x1b[?25h\x1b[?1049l"));
    assert_eq!(
        r.out.matches("\x1b[2J").count(),
        2,
        "drew twice in three polls"
    );
    // On a terminal the status is coloured, as the script's is.
    assert!(r.out.contains("\x1b[1mstash\x1b[0m"));

    // An error mid-watch still restores the screen.
    let mut host = FakeHost::new(w.now);
    let dir = w.ultron().join("stash");
    host.naps
        .push_back(Box::new(move || fs::remove_dir_all(&dir).unwrap()));
    let r = run_in(&ctx, &mut host, &["watch", "stash"]);
    assert!(r.result.is_err());
    assert!(r.out.ends_with("\x1b[?25h\x1b[?1049l"));
}

#[test]
fn fingerprint_is_stable_for_an_untouched_program() {
    let w = World::fixtures();
    let dir = w.ultron().join("standards-hosted");
    let dir = dir.to_str().unwrap();
    assert_eq!(fingerprint(dir), fingerprint(dir));
    assert!(fingerprint(dir).contains("/tasks/13-cli-ultron.md:"));
}

// ── colour, stale ledgers, log, gc, --json ───────────────────────────────────────

#[test]
fn colours_on_a_terminal_match_the_script() {
    let w = World::fixtures();
    let ctx = w.ctx(false, true);
    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["ls"]);
    assert!(r.out.starts_with(
        "\x1b[1mPROGRAM                   TASKS  LANDED  ACTIVE  BLOCKED  GATED  LAST\x1b[0m\n"
    ));
    assert!(r.out.contains(
        "stash                        21      15       0 \x1b[33m       5\x1b[0m \x1b[33m     7\x1b[0m  \x1b[2m25d ago\x1b[0m\n"
    ));
    assert!(
        r.out.contains("     0\x1b[0m"),
        "uncoloured counts still reset"
    );
    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["status", "stash"]);
    assert!(
        r.out
            .contains("STASH-8      \x1b[33mqueued \u{1f512}    \x1b[0m")
    );
    assert!(r.out.contains("STASH-1      \x1b[32mlanded       \x1b[0m"));
    assert!(r.out.contains("\n\x1b[33mwaiting on you:\x1b[0m STASH-3 "));
}

#[test]
fn a_journal_for_a_queued_task_flags_a_stale_ledger() {
    let w = World::empty();
    let dir = w.ultron().join("p");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("PROGRAM.md"),
        "| AB-1 | queued |\n| AB-2 | queued |\n- AB-3 dispatched\n",
    )
    .unwrap();
    fs::write(dir.join("AB-1.md"), "").unwrap();
    fs::write(
        dir.join("AB-3.md"),
        "PLAN\n- [x] a\n- [ ] b\nSTATE: on step 2\n",
    )
    .unwrap();
    let r = run_text(&w, &["status", "p"]);
    assert!(
        r.out
            .ends_with("\n! journal exists but the ledger still says queued: AB-1 \n"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .contains("AB-3         dispatched       1/2  just now  on step 2\n")
    );
}

#[test]
fn log_prints_every_match_and_a_missing_last_newline() {
    let w = World::empty();
    for p in ["a", "b"] {
        fs::create_dir_all(w.ultron().join(p)).unwrap();
        fs::write(w.ultron().join(p).join("ZZ-1.md"), format!("from {p}")).unwrap();
    }
    let r = run_text(&w, &["log", "ZZ-1"]);
    let root = w.ultron_str();
    assert_eq!(
        r.out,
        format!(
            "\u{2500}\u{2500} {root}/a/ZZ-1.md\nfrom a\n\u{2500}\u{2500} {root}/b/ZZ-1.md\nfrom b\n"
        )
    );
    let r = run_in(
        &w.ctx(true, false),
        &mut FakeHost::new(w.now),
        &["log", "ZZ-1"],
    );
    assert_eq!(r.out, "");
    assert_eq!(data(&r)["journals"][1]["content"], "from b");
}

#[test]
fn gc_always_prompts_and_deletes_only_on_yes() {
    let w = World::fixtures();
    let dir = w.ultron().join("stash");

    for reply in [Some("n"), Some(""), None] {
        let mut host = FakeHost::new(w.now);
        host.input.extend(reply.map(str::to_owned));
        let r = run_in(&w.ctx(false, false), &mut host, &["gc", "stash"]);
        assert_eq!(
            r.out,
            format!(
                "! stash is 15/21 landed \u{2014} not finished.\ndelete {} ? [y/N] kept.\n",
                dir.display()
            )
        );
        assert!(dir.is_dir());
    }

    // Fully landed: no warning, still a prompt.
    let done = w.ultron().join("done");
    fs::create_dir_all(&done).unwrap();
    fs::write(done.join("PROGRAM.md"), "- XY-1 landed\n- XY-2 done\n").unwrap();
    let mut host = FakeHost::new(w.now);
    host.input.push_back("Y".into());
    let r = run_in(&w.ctx(false, false), &mut host, &["gc", "done"]);
    assert_eq!(
        r.out,
        format!("delete {} ? [y/N] \u{2713} removed done\n", done.display())
    );
    assert!(!done.exists());

    // --json: the prompt on stderr, the result as data.
    let mut host = FakeHost::new(w.now);
    host.input.push_back("y".into());
    let r = run_in(&w.ctx(true, false), &mut host, &["gc", "stash"]);
    assert_eq!(r.out, "");
    assert!(r.err.contains("[y/N]"));
    assert_eq!(data(&r)["removed"], true);
    assert_eq!(data(&r)["landed"], 15);
    assert!(!dir.exists());
}

#[test]
fn json_is_structured_and_prints_nothing_itself() {
    let w = World::fixtures();
    let ctx = w.ctx(true, true);
    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["ls"]);
    assert_eq!(r.out, "");
    let v = data(&r);
    assert_eq!(v["state_dir"], w.ultron_str());
    assert_eq!(v["programs"][0]["slug"], "standards-hosted");
    assert_eq!(v["programs"][3]["slug"], "stash");
    assert_eq!(v["programs"][3]["blocked"], 5);
    assert_eq!(v["programs"][3]["last"], "25d ago");

    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["status", "stash"]);
    assert_eq!(r.out, "");
    let v = data(&r);
    assert_eq!(v["has_brief"], true);
    assert_eq!(v["orphans"], serde_json::json!(["HOMELAB-cutover"]));
    let t = &v["tasks"][3];
    assert_eq!(t["id"], "STASH-10");
    assert_eq!(t["state"], "landed");
    assert_eq!(t["plan"], serde_json::json!({"done": 15, "total": 15}));
    assert_eq!(t["journal_age"], "28d ago");
    assert_eq!(v["tasks"][0]["journal"], serde_json::Value::Null);

    // watch under --json is status once, even on a terminal.
    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["watch", "stash"]);
    assert_eq!(data(&r)["slug"], "stash");
    let r = run_in(&ctx, &mut FakeHost::new(w.now), &["help"]);
    assert_eq!(r.out, "");
    assert!(data(&r)["usage"].as_str().unwrap().contains("ultron gc"));
}
