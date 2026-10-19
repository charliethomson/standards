//! `standards doctor` against the real server in-process, a scratch HOME, scratch git
//! repos, and a fake `claude` on a scratch PATH that records its argv and edits the
//! scratch `~/.claude.json` the way `claude mcp add/remove` would.

mod harness;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use harness::{Run, Scratch, dead_host, run, serve, server};
use poem::web::Data;
use poem::{EndpointExt as _, Route, get, handler};
use serde_json::{Value, json};

const FAKE_CLAUDE: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/claude-argv.log"
case "$1 $2" in
  "mcp add")
    for a; do url="$a"; done
    printf '{"oauthAccount":{"accessToken":"secret"},"mcpServers":{"standards":{"type":"http","url":"%s"}}}\n' "$url" > "$HOME/.claude.json" ;;
  "mcp remove")
    printf '{"oauthAccount":{"accessToken":"secret"},"mcpServers":{}}\n' > "$HOME/.claude.json" ;;
esac
"#;

/// A scratch machine with the fake `claude` on PATH and a registry nobody answers.
fn machine() -> Scratch {
    let mut s = Scratch::new();
    let bin = s.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude");
    std::fs::write(&claude, FAKE_CLAUDE).unwrap();
    chmod(&claude, 0o755);
    s.env.path = Some(bin.into_os_string());
    s.env.config.registry_url = Some(dead_host());
    s
}

#[cfg(unix)]
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
fn symlink(target: &str, link: &Path) {
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, link).unwrap();
}

fn doctor(s: &Scratch, host: &str, extra: &[&str]) -> Run {
    let mut args = vec!["--host", host, "doctor"];
    args.extend_from_slice(extra);
    run(&s.env, &args)
}

fn doctor_json(s: &Scratch, host: &str, extra: &[&str]) -> (u8, Value) {
    let mut args = vec!["--json", "--host", host, "doctor"];
    args.extend_from_slice(extra);
    let r = run(&s.env, &args);
    (r.code, r.json())
}

/// `{id: check}` from a `--json` run.
fn checks(v: &Value) -> serde_json::Map<String, Value> {
    v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_owned(), c.clone()))
        .collect()
}

fn status(v: &Value, id: &str) -> String {
    checks(v)[id]["status"].as_str().unwrap().to_owned()
}

fn argv_log(s: &Scratch) -> String {
    std::fs::read_to_string(s.env.home.join("claude-argv.log")).unwrap_or_default()
}

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn served_skills() -> Vec<String> {
    let manifest: Value = reqwest::blocking::get(format!("{}/manifest.json", server()))
        .unwrap()
        .json()
        .unwrap();
    manifest["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect()
}

// ── user scope ──────────────────────────────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines)] // one scenario, end to end
fn a_legacy_machine_is_reported_fixed_and_then_clean() {
    let s = machine();
    let home = s.env.home.clone();
    let skills = home.join(".claude/skills");
    // Legacy links, a served name and one no longer served; an unmarked dir; an orphan stub.
    symlink(
        "/old/git/standards/skills/thmsn-ultron",
        &skills.join("thmsn-ultron"),
    );
    symlink(
        "/old/git/standards/skills/thmsn-standards-sync",
        &skills.join("thmsn-standards-sync"),
    );
    std::fs::create_dir_all(skills.join("shadcn")).unwrap();
    std::fs::write(skills.join("shadcn/SKILL.md"), "mine").unwrap();
    std::fs::create_dir_all(skills.join("thmsn-gone")).unwrap();
    std::fs::write(skills.join("thmsn-gone/.standards-managed"), "x").unwrap();
    symlink("elsewhere/vision", &skills.join("vision"));
    // Old PATH links, and a real binary that is not one.
    symlink(
        "/old/git/standards/bin/ultron",
        &home.join(".local/bin/ultron"),
    );
    symlink(
        "/old/git/standards/bin/janitor",
        &home.join(".local/bin/janitor"),
    );
    std::fs::write(home.join(".local/bin/standards"), "binary").unwrap();
    // Settings with keys and hooks of their own.
    let settings = home.join(".claude/settings.json");
    std::fs::write(
        &settings,
        r#"{"model": "opus", "hooks": {"Stop": [{"hooks": []}]}, "zzz": [1, 2]}"#,
    )
    .unwrap();

    let (code, v) = doctor_json(&s, server(), &[]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["status"], "fail");
    let ids: Vec<&str> = v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "mcp.registered",
            "mcp.reachable",
            "skills.synced",
            "hook.session-start",
            "cli.current",
            "legacy.path-links"
        ]
    );
    let c = checks(&v);
    assert_eq!(c["mcp.registered"]["status"], "fail");
    assert_eq!(c["mcp.registered"]["fixable"], true);
    assert_eq!(c["mcp.reachable"]["status"], "ok");
    assert_eq!(c["skills.synced"]["status"], "fail");
    let details = c["skills.synced"]["details"].to_string();
    assert!(details.contains("thmsn-ultron: legacy link"), "{details}");
    assert!(details.contains("thmsn-standards-sync: legacy link, no longer served"));
    assert!(details.contains("thmsn-gone: no longer served"));
    assert!(details.contains("thmsn-standards: missing"));
    assert!(!details.contains("shadcn") && !details.contains("vision"));
    assert_eq!(c["hook.session-start"]["status"], "fail");
    assert_eq!(c["cli.current"]["status"], "ok");
    assert!(
        c["cli.current"]["message"]
            .as_str()
            .unwrap()
            .contains("latest unknown")
    );
    assert_eq!(c["legacy.path-links"]["status"], "fail");
    assert_eq!(
        c["legacy.path-links"]["details"].as_array().unwrap().len(),
        2
    );
    assert_eq!(v["data"]["repo"], Value::Null);
    assert_eq!(argv_log(&s), "", "a plain run fixes nothing");

    // Human output: terse lines, the summary names the fix.
    let r = doctor(&s, server(), &[]);
    assert_eq!(r.code, 1);
    assert!(
        r.out
            .contains("  fail  mcp.registered     not registered (fixable)"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .ends_with("4 failed; `standards doctor --fix` repairs 4\n"),
        "{}",
        r.out
    );

    let r = doctor(&s, server(), &["--fix"]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("  fixed mcp.registered"), "{}", r.out);
    assert!(r.out.ends_with("4 fixed\n"), "{}", r.out);
    let url = format!("{}/mcp", server());
    assert_eq!(
        argv_log(&s),
        format!("mcp add -s user -t http standards {url}\n")
    );

    // Stubs: one per served skill, the spec text plus the marker.
    let served = served_skills();
    assert!(served.contains(&"thmsn-ultron".to_owned()));
    for name in &served {
        let dir = skills.join(name);
        assert!(!dir.is_symlink(), "{name}");
        let stub = read(dir.join("SKILL.md"));
        assert!(stub.starts_with(&format!("---\nname: {name}\ndescription: ")));
        assert!(stub.contains(&format!(
            "Call `get_skill(\"{name}\")` on the `standards` MCP"
        )));
        let marker = read(dir.join(".standards-managed"));
        assert!(marker.starts_with("content_sha256=") && marker.contains("\ndescription_sha256="));
    }
    assert!(
        !skills.join("thmsn-standards-sync").exists()
            && !skills.join("thmsn-standards-sync").is_symlink()
    );
    assert!(!skills.join("thmsn-gone").exists());
    assert_eq!(read(skills.join("shadcn/SKILL.md")), "mine");
    assert!(skills.join("vision").is_symlink());
    // PATH links gone, the real binary kept.
    assert!(!home.join(".local/bin/ultron").is_symlink());
    assert_eq!(read(home.join(".local/bin/standards")), "binary");
    // Settings: merged, order kept, backed up.
    let merged: Value = serde_json::from_str(&read(&settings)).unwrap();
    let keys: Vec<&String> = merged.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["model", "hooks", "zzz"]);
    assert_eq!(
        merged["hooks"]["SessionStart"],
        json!([{"matcher": "startup", "hooks": [{"type": "command", "command": "standards doctor --quiet", "timeout": 10}]}])
    );
    assert_eq!(merged["hooks"]["Stop"], json!([{"hooks": []}]));
    assert!(read(home.join(".claude/settings.json.bak")).contains("\"opus\""));

    // Clean now; another --fix changes nothing.
    let r = doctor(&s, server(), &[]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.ends_with("all checks ok\n"), "{}", r.out);
    let r = doctor(&s, server(), &["--fix"]);
    assert_eq!(r.code, 0);
    assert!(r.out.ends_with("all checks ok\n"), "{}", r.out);
    let merged: Value = serde_json::from_str(&read(&settings)).unwrap();
    assert_eq!(merged["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(argv_log(&s).lines().count(), 1);

    // The hook's run is silent when healthy.
    let r = doctor(&s, server(), &["--quiet"]);
    assert_eq!((r.code, r.out.as_str(), r.err.as_str()), (0, "", ""));

    // A stale stub (the server's description changed) is rewritten.
    std::fs::write(
        skills.join("thmsn-ultron/.standards-managed"),
        "content_sha256=old\n",
    )
    .unwrap();
    let (code, v) = doctor_json(&s, server(), &["--only", "skills.synced"]);
    assert_eq!(code, 1);
    assert!(
        checks(&v)["skills.synced"]["details"]
            .to_string()
            .contains("thmsn-ultron: out of date")
    );
    let r = doctor(&s, server(), &["--quiet"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out,
        "standards: skills.synced needs attention; run `standards doctor --fix`\n"
    );
    let (code, _) = doctor_json(&s, server(), &["--fix", "--only", "skills.synced"]);
    assert_eq!(code, 0);
}

#[test]
fn a_stale_registration_is_removed_then_added() {
    let s = machine();
    std::fs::write(
        s.env.home.join(".claude.json"),
        r#"{"mcpServers": {"standards": {"type": "stdio", "command": "standards/mcp/x"}}}"#,
    )
    .unwrap();
    let (code, v) = doctor_json(&s, server(), &["--only", "mcp.registered"]);
    assert_eq!(code, 1);
    assert_eq!(
        checks(&v)["mcp.registered"]["message"],
        format!(
            "registered as stdio standards/mcp/x, want http {}/mcp",
            server()
        )
    );
    let (code, v) = doctor_json(&s, server(), &["--fix", "--only", "mcp.registered"]);
    assert_eq!(code, 0, "{v}");
    let c = &checks(&v)["mcp.registered"];
    assert_eq!(c["fixed"], true);
    assert_eq!(c["status"], "ok");
    let log = argv_log(&s);
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines,
        [
            "mcp remove -s user standards".to_owned(),
            format!("mcp add -s user -t http standards {}/mcp", server())
        ]
    );
}

#[test]
fn without_claude_on_path_registration_fails_unfixed() {
    let mut s = machine();
    s.env.path = Some(s.path().join("nowhere").into_os_string());
    let (code, v) = doctor_json(&s, server(), &["--fix", "--only", "mcp.registered"]);
    assert_eq!(code, 1);
    let c = &checks(&v)["mcp.registered"];
    assert_eq!(c["status"], "fail");
    assert_eq!(c["fixable"], false);
    assert_eq!(c["fixed"], false);
    assert!(
        c["message"]
            .as_str()
            .unwrap()
            .contains("`claude` is not on PATH")
    );

    // A claude that fails: the fix reports why.
    let bin = s.path().join("bad");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("claude"), "#!/bin/sh\necho nope >&2\nexit 1\n").unwrap();
    chmod(&bin.join("claude"), 0o755);
    s.env.path = Some(bin.into_os_string());
    let (code, v) = doctor_json(&s, server(), &["--fix", "--only", "mcp.registered"]);
    assert_eq!(code, 1);
    assert_eq!(
        checks(&v)["mcp.registered"]["message"],
        format!(
            "fix failed: claude mcp add -s user -t http standards {}/mcp: nope",
            server()
        )
    );

    // An unreadable ~/.claude.json is reported, not touched.
    std::fs::write(s.env.home.join(".claude.json"), "{oops").unwrap();
    let (_, v) = doctor_json(&s, server(), &["--only", "mcp.registered"]);
    assert_eq!(status(&v, "mcp.registered"), "fail");
    assert_eq!(read(s.env.home.join(".claude.json")), "{oops");
}

#[test]
fn broken_settings_are_left_for_a_human() {
    let s = machine();
    let settings = s.env.home.join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    for bad in ["{nope", "[1]", r#"{"hooks": []}"#] {
        std::fs::write(&settings, bad).unwrap();
        let (code, v) = doctor_json(&s, server(), &["--fix", "--only", "hook.session-start"]);
        assert_eq!(code, 1, "{bad}");
        assert_eq!(status(&v, "hook.session-start"), "fail");
        assert_eq!(read(&settings), bad);
    }
    // No settings.json at all: created.
    std::fs::remove_file(&settings).unwrap();
    let (code, _) = doctor_json(&s, server(), &["--fix", "--only", "hook.session-start"]);
    assert_eq!(code, 0);
    assert!(read(&settings).contains("standards doctor --quiet"));
    assert!(!s.env.home.join(".claude/settings.json.bak").exists());
}

// ── cli.current, against a fake server and registry ────────────────────────

#[derive(Clone)]
struct Versions {
    min: &'static str,
    latest: &'static str,
}

#[handler]
fn fake_manifest(Data(v): Data<&Arc<Versions>>) -> poem::web::Json<Value> {
    poem::web::Json(json!({
        "schema": 1, "commit": "abcdef1234", "server_version": "0.1.9",
        "min_cli_version": v.min, "cli_install_url": "https://x/install",
        "archetypes": ["library"], "skills": []
    }))
}

#[handler]
fn fake_resolve(Data(v): Data<&Arc<Versions>>) -> poem::web::Json<Value> {
    poem::web::Json(json!({
        "version": v.latest, "target": "linux-amd64", "download_url": "/d",
        "file_name": "standards", "sha256": "00"
    }))
}

fn fake(min: &'static str, latest: &'static str) -> String {
    let versions = Arc::new(Versions { min, latest });
    serve(
        Route::new()
            .at("/manifest.json", get(fake_manifest))
            .at(
                "/api/v1/products/dev.thmsn.standards/apps/cli/resolve",
                get(fake_resolve),
            )
            .data(versions),
    )
}

#[test]
fn cli_current_compares_with_the_minimum_and_the_latest() {
    let mut s = machine();
    let host = fake("0.2.0", "0.2.5");
    s.env.config.registry_url = Some(host.clone());

    let (code, v) = doctor_json(&s, &host, &["--only", "cli.current"]);
    assert_eq!(code, 1);
    let c = &checks(&v)["cli.current"];
    assert_eq!(
        c["message"],
        "0.1.0 is older than the server's minimum 0.2.0"
    );
    assert_eq!(c["fixable"], true);
    // The fix is self-update; with no running binary to replace, it fails and says so.
    let (code, v) = doctor_json(&s, &host, &["--fix", "--only", "cli.current"]);
    assert_eq!(code, 1);
    assert!(
        checks(&v)["cli.current"]["message"]
            .as_str()
            .unwrap()
            .starts_with("fix failed: "),
        "{v}"
    );

    s.env.version = "0.2.1".into();
    let (code, v) = doctor_json(&s, &host, &["--only", "cli.current"]);
    assert_eq!(code, 0);
    assert_eq!(status(&v, "cli.current"), "warn");
    assert_eq!(
        checks(&v)["cli.current"]["message"],
        "0.2.1; 0.2.5 is available"
    );
    // --quiet never asks the registry.
    let r = doctor(&s, &host, &["--quiet", "--only", "cli.current"]);
    assert_eq!((r.code, r.out.as_str()), (0, ""));

    s.env.version = "0.2.5".into();
    let (_, v) = doctor_json(&s, &host, &["--only", "cli.current"]);
    assert_eq!(checks(&v)["cli.current"]["message"], "0.2.5 is the latest");

    s.env.version = "dev".into();
    let (code, v) = doctor_json(&s, &host, &["--only", "cli.current"]);
    assert_eq!((code, status(&v, "cli.current")), (0, "warn".to_owned()));

    let host = fake("x.y", "0.0.1");
    s.env.version = "0.1.0".into();
    let (_, v) = doctor_json(&s, &host, &["--only", "cli.current"]);
    assert_eq!(status(&v, "cli.current"), "warn");
}

// ── flags, exit codes ───────────────────────────────────────────────────────

#[test]
fn unreachable_server_exits_three_unless_quiet() {
    let s = machine();
    let dead = dead_host();
    let r = doctor(&s, &dead, &[]);
    assert_eq!(r.code, 3, "{}{}", r.out, r.err);
    assert!(r.err.starts_with("error: can't reach "), "{}", r.err);

    // Nothing selected needs the manifest: reachability is only a warning.
    let (code, v) = doctor_json(&s, &dead, &["--only", "mcp.reachable,legacy.path-links"]);
    assert_eq!(code, 0);
    assert_eq!(v["status"], "warn");
    assert_eq!(status(&v, "mcp.reachable"), "warn");

    // --quiet: always 0, one line about what it could still check.
    let r = doctor(&s, &dead, &["--quiet"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out,
        "standards: mcp.registered, hook.session-start need attention; run `standards doctor --fix`\n"
    );
    // ...and silent about everything when it can't even parse its flags' ids.
    let r = doctor(&s, &dead, &["--quiet", "--only", "bogus"]);
    assert_eq!((r.code, r.out.as_str()), (0, ""));
}

#[test]
fn quiet_uses_the_cached_manifest() {
    let s = machine();
    // A normal run caches the manifest...
    let (code, _) = doctor_json(&s, server(), &["--fix"]);
    assert_eq!(code, 0);
    let cached = s.env.home.join(".cache/thmsn/standards/manifest.json");
    let mut c: Value = serde_json::from_str(&read(&cached)).unwrap();
    // ...which --quiet trusts for an hour: a skill only the cache knows is reported.
    c["manifest"]["skills"][0]["content_sha256"] = json!("changed");
    std::fs::write(&cached, c.to_string()).unwrap();
    let r = doctor(&s, server(), &["--quiet"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out,
        "standards: skills.synced needs attention; run `standards doctor --fix`\n"
    );
}

#[test]
fn usage_errors_exit_two() {
    let s = machine();
    for args in [
        &["--only", "nope"][..],
        &["--fix", "--quiet"],
        &["--repo", "/definitely/not/a/repo"],
        &["--only"],
    ] {
        let r = doctor(&s, server(), args);
        assert_eq!(r.code, 2, "{args:?}: {}{}", r.out, r.err);
    }
    let r = doctor(&s, server(), &["--only", "nope"]);
    assert!(
        r.err
            .contains("unknown check `nope`; one of: mcp.registered"),
        "{}",
        r.err
    );
    // --ci outside a repo has nothing to check.
    let r = doctor(&s, server(), &["--ci"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("not in a git repo"), "{}", r.err);
}

// ── repo scope ──────────────────────────────────────────────────────────────

/// `git` for test setup: no global or system config, no signing.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// An empty repo under the work dir, made the cwd.
fn fresh_repo(s: &mut Scratch, name: &str) -> PathBuf {
    let root = s.env.cwd.join(name);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    s.env.cwd.clone_from(&root);
    root
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const TEMPLATE_MCP: &str = r#"{
  "//": "Root .mcp.json for a repo that vendors the standards submodule.",
  "mcpServers": {
  }
}
"#;

const WOODPECKER: &str = "clone:
  git:
    image: woodpeckerci/plugin-git
    settings:
      partial: false
      submodule_override:
        standards: https://github.com/charliethomson/standards.git
steps: []
";

/// A repo shaped like ~/git/libid before migration: the standards submodule (from a local
/// source repo), tracked skill links into it, the template .mcp.json and a CI override.
fn legacy_repo(s: &mut Scratch) -> PathBuf {
    let src = s.path().join("standards-src");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-q"]);
    write(&src, "skills/thmsn-standards/SKILL.md", "x");
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "src"]);

    let root = s.env.cwd.join("libid");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    write(
        &root,
        ".standards.conf",
        "PRODUCT=id\nPRODUCT_UPPER=ID\nLIB=libid\nARCHETYPE=library\n",
    );
    write(
        &root,
        "AGENTS.md",
        "# AGENTS\n\nRead standards/AGENTS.md.\n",
    );
    write(&root, ".mcp.json", TEMPLATE_MCP);
    write(&root, ".woodpecker/rust.ci.yml", WOODPECKER);
    write(
        &root,
        ".github/workflows/ci.yml",
        "steps:\n  - uses: actions/checkout@v4\n    with:\n      submodules: true\n",
    );
    git(
        &root,
        &["submodule", "add", "-q", src.to_str().unwrap(), "standards"],
    );
    symlink(
        "../../standards/skills/thmsn-standards",
        &root.join(".claude/skills/thmsn-standards"),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "legacy"]);
    // One more link, never committed.
    symlink(
        "../../standards/skills/thmsn-standards-sync",
        &root.join(".claude/skills/thmsn-standards-sync"),
    );
    s.env.cwd.clone_from(&root);
    root
}

#[test]
#[allow(clippy::too_many_lines)] // one scenario, end to end
fn a_legacy_repo_is_reported_fixed_staged_and_never_committed() {
    let mut s = machine();
    let root = legacy_repo(&mut s);
    let head = git(&root, &["rev-parse", "HEAD"]);

    let (code, v) = doctor_json(&s, server(), &["--ci"]);
    assert_eq!(code, 1, "{v}");
    let ids: Vec<&str> = v["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "repo.conf",
            "repo.agents",
            "legacy.submodule",
            "legacy.skill-links",
            "legacy.mcp-json",
            "legacy.ci-override"
        ],
        "--ci skips user scope"
    );
    let c = checks(&v);
    assert_eq!(c["repo.conf"]["status"], "ok");
    assert_eq!(c["repo.agents"]["status"], "fail");
    assert_eq!(c["legacy.submodule"]["status"], "fail");
    assert_eq!(
        c["legacy.submodule"]["details"],
        json!([
            ".gitmodules: [submodule \"standards\"]",
            ".git/modules/standards"
        ])
    );
    assert_eq!(
        c["legacy.skill-links"]["details"].as_array().unwrap().len(),
        2
    );
    assert_eq!(c["legacy.mcp-json"]["status"], "fail");
    assert_eq!(c["legacy.ci-override"]["status"], "warn");
    assert_eq!(c["legacy.ci-override"]["fixable"], false);
    assert_eq!(
        c["legacy.ci-override"]["details"],
        json!([
            ".woodpecker/rust.ci.yml:7: standards: https://github.com/charliethomson/standards.git",
            ".github/workflows/ci.yml:4: submodules: true"
        ])
    );

    // From a subdirectory, with the user scope too: both scopes, in order.
    let r = doctor(&s, server(), &[]);
    assert_eq!(r.code, 1);
    let user_at = r.out.find("user\n").unwrap();
    let repo_at = r.out.find(&format!("repo {}", root.display())).unwrap();
    assert!(user_at < repo_at, "{}", r.out);
    assert!(
        r.out.contains("          .woodpecker/rust.ci.yml:7: "),
        "{}",
        r.out
    );

    let r = doctor(&s, server(), &["--ci", "--fix"]);
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    assert!(r.out.ends_with("4 fixed, 1 warning\n"), "{}", r.out);

    // Staged, not committed.
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    let staged = git(&root, &["diff", "--cached", "--name-status"]);
    let mut staged: Vec<&str> = staged.lines().collect();
    staged.sort_unstable();
    assert_eq!(
        staged,
        [
            "D\t.claude/skills/thmsn-standards",
            "D\t.gitmodules",
            "D\t.mcp.json",
            "D\tstandards",
            "M\tAGENTS.md",
        ]
    );
    assert_eq!(
        git(&root, &["status", "--porcelain", "--untracked-files=all"])
            .lines()
            .filter(|l| !l.starts_with(['D', 'M']))
            .count(),
        0
    );
    assert!(!root.join("standards").exists());
    assert!(!root.join(".git/modules/standards").exists());
    assert!(!root.join(".claude/skills").exists());
    assert!(read(root.join("AGENTS.md")).starts_with(
        "Standards: archetype=library product=id — rules via the `standards` MCP (https://standards.thmsn.dev). Repo-specific overrides below.\n\n# AGENTS\n"
    ));
    assert_eq!(read(root.join(".woodpecker/rust.ci.yml")), WOODPECKER);

    let (code, v) = doctor_json(&s, server(), &["--ci"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "warn");
    for id in [
        "repo.conf",
        "repo.agents",
        "legacy.submodule",
        "legacy.skill-links",
        "legacy.mcp-json",
    ] {
        assert_eq!(status(&v, id), "ok", "{id}");
    }
}

#[test]
fn a_repo_with_other_submodules_keeps_them() {
    let mut s = machine();
    let root = fresh_repo(&mut s, "multi");
    write(
        &root,
        ".standards.conf",
        "PRODUCT=multi\nARCHETYPE=cli-tool\n",
    );
    write(
        &root,
        ".gitmodules",
        "[submodule \"standards\"]\n\tpath = standards\n\turl = x\n[submodule \"vendor\"]\n\tpath = vendor\n\turl = y\n",
    );
    write(
        &root,
        ".github/workflows/ci.yml",
        "    with:\n      submodules: recursive\n",
    );
    git(&root, &["add", "-A"]);
    // The standards section has no gitlink: the fix edits .gitmodules instead of git rm.
    let (code, v) = doctor_json(
        &s,
        server(),
        &[
            "--ci",
            "--fix",
            "--only",
            "legacy.submodule,legacy.ci-override",
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(status(&v, "legacy.submodule"), "ok");
    assert_eq!(
        status(&v, "legacy.ci-override"),
        "ok",
        "another submodule needs the checkout flag"
    );
    let modules = read(root.join(".gitmodules"));
    assert!(
        !modules.contains("standards") && modules.contains("vendor"),
        "{modules}"
    );
}

#[test]
fn repo_agents_rewrites_a_stale_line_and_creates_a_missing_file() {
    let mut s = machine();
    let root = fresh_repo(&mut s, "tool");
    write(
        &root,
        ".standards.conf",
        "PRODUCT=tool\nARCHETYPE=cli-tool\n",
    );
    write(
        &root,
        "AGENTS.md",
        "Standards: archetype=library product=tool — old\n\n# AGENTS\n",
    );
    let (code, v) = doctor_json(&s, server(), &["--ci", "--only", "repo.agents"]);
    assert_eq!(code, 1);
    assert!(
        checks(&v)["repo.agents"]["details"][0]
            .as_str()
            .unwrap()
            .starts_with("have: Standards: archetype=library")
    );
    let (code, _) = doctor_json(&s, server(), &["--ci", "--fix", "--only", "repo.agents"]);
    assert_eq!(code, 0);
    assert_eq!(
        read(root.join("AGENTS.md")),
        "Standards: archetype=cli-tool product=tool — rules via the `standards` MCP (https://standards.thmsn.dev). Repo-specific overrides below.\n\n# AGENTS\n"
    );
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]),
        "AGENTS.md\n"
    );

    std::fs::remove_file(root.join("AGENTS.md")).unwrap();
    let (code, v) = doctor_json(&s, server(), &["--ci", "--fix", "--only", "repo.agents"]);
    assert_eq!(code, 0, "{v}");
    assert!(read(root.join("AGENTS.md")).ends_with("Repo-specific overrides below.\n"));
}

#[test]
fn repo_conf_failures() {
    let mut s = machine();
    let root = fresh_repo(&mut s, "r");

    // No conf and no leftovers: not a standards repo, skipped outside --ci.
    let (code, v) = doctor_json(&s, server(), &["--only", "repo.conf,repo.agents"]);
    assert_eq!(code, 0);
    assert_eq!(v["data"]["checks"], json!([]));
    assert!(
        v["data"]["repo_skipped"]
            .as_str()
            .unwrap()
            .contains("standards init")
    );
    let r = doctor(&s, server(), &["--only", "repo.conf"]);
    assert!(r.out.contains("  skip  no .standards.conf"), "{}", r.out);
    // ...but --ci gates on it.
    let (code, v) = doctor_json(&s, server(), &["--ci"]);
    assert_eq!(code, 1);
    assert_eq!(
        checks(&v)["repo.conf"]["message"],
        "no .standards.conf; run `standards init`"
    );
    assert_eq!(status(&v, "repo.agents"), "skip");

    for (conf, want) in [
        ("junk\n", ".standards.conf line 1: expected KEY=VALUE"),
        ("PRODUCT=r\n", ".standards.conf has no ARCHETYPE; add one"),
    ] {
        write(&root, ".standards.conf", conf);
        let (code, v) = doctor_json(&s, server(), &["--ci", "--only", "repo.conf"]);
        assert_eq!(code, 1);
        assert_eq!(checks(&v)["repo.conf"]["message"], want);
    }
    write(&root, ".standards.conf", "PRODUCT=r\nARCHETYPE=spaceship\n");
    let (_, v) = doctor_json(&s, server(), &["--ci", "--only", "repo.conf"]);
    let message = checks(&v)["repo.conf"]["message"].to_string();
    assert!(
        message.starts_with("\"unknown archetype `spaceship` in .standards.conf; one of: "),
        "{message}"
    );

    // --repo points at another repo from anywhere.
    let elsewhere = s.path().join("home");
    s.env.cwd = elsewhere;
    let (code, v) = doctor_json(
        &s,
        server(),
        &[
            "--ci",
            "--repo",
            root.to_str().unwrap(),
            "--only",
            "repo.conf",
        ],
    );
    assert_eq!(code, 1);
    assert_eq!(v["data"]["repo"], root.display().to_string());
}

#[test]
fn mcp_json_entries_into_standards_are_dropped_and_the_rest_kept() {
    let mut s = machine();
    let root = fresh_repo(&mut s, "m");
    write(&root, ".standards.conf", "PRODUCT=m\nARCHETYPE=library\n");
    write(
        &root,
        ".mcp.json",
        r#"{"mcpServers": {"grafana": {"command": "standards/mcp/grafana/run"}, "mine": {"command": "npx", "args": ["x"]}}, "z": 1}"#,
    );
    let (code, v) = doctor_json(&s, server(), &["--ci", "--only", "legacy.mcp-json"]);
    assert_eq!(code, 1);
    assert_eq!(checks(&v)["legacy.mcp-json"]["details"], json!(["grafana"]));
    let (code, _) = doctor_json(
        &s,
        server(),
        &["--ci", "--fix", "--only", "legacy.mcp-json"],
    );
    assert_eq!(code, 0);
    let after: Value = serde_json::from_str(&read(root.join(".mcp.json"))).unwrap();
    assert_eq!(
        after,
        json!({"mcpServers": {"mine": {"command": "npx", "args": ["x"]}}, "z": 1})
    );
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]),
        ".mcp.json\n"
    );

    // Only legacy entries under a `//` note: the whole file goes.
    write(
        &root,
        ".mcp.json",
        r#"{"//": "note", "mcpServers": {"k": {"command": "./standards/mcp/k"}}}"#,
    );
    let (code, _) = doctor_json(
        &s,
        server(),
        &["--ci", "--fix", "--only", "legacy.mcp-json"],
    );
    assert_eq!(code, 0);
    assert!(!root.join(".mcp.json").exists());

    write(&root, ".mcp.json", "{broken");
    let (code, v) = doctor_json(
        &s,
        server(),
        &["--ci", "--fix", "--only", "legacy.mcp-json"],
    );
    assert_eq!(
        (code, status(&v, "legacy.mcp-json")),
        (0, "warn".to_owned())
    );
    assert_eq!(read(root.join(".mcp.json")), "{broken");
}
