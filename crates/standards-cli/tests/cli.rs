//! The surface, `init` and `new` against the real server running in-process.

mod harness;

use harness::{Scratch, dead_host, run, server, tree};
use standards_core::StandardsConf;

fn read(path: impl AsRef<std::path::Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn help_and_version_exit_zero_usage_errors_exit_two() {
    let s = Scratch::new();
    let r = run(&s.env, &["--help"]);
    assert_eq!(r.code, 0);
    for cmd in ["doctor", "init", "new", "self-update", "janitor", "ultron"] {
        assert!(r.out.contains(cmd), "{cmd} missing from help");
    }
    assert!(r.out.contains("STANDARDS_HOST"));
    let r = run(&s.env, &["--version"]);
    assert_eq!(r.code, 0);
    assert!(r.out.starts_with("standards "));

    for args in [
        &[][..],
        &["lint"],
        &["new", "library"],
        &["init", "--bogus"],
    ] {
        let r = run(&s.env, args);
        assert_eq!(r.code, 2, "{args:?}: {}", r.err);
        assert_eq!(r.out, "");
    }
}

#[test]
fn unimplemented_subcommands_fail_with_one() {
    let s = Scratch::new();
    for (cmd, rest) in [
        ("doctor", &["--fix", "--quiet"][..]),
        ("janitor", &["clean", "-y"]),
        ("ultron", &["watch"]),
    ] {
        let mut args = vec![cmd];
        args.extend(rest);
        let r = run(&s.env, &args);
        assert_eq!(r.code, 1, "{cmd}");
        assert_eq!(r.err, format!("error: {cmd}: not implemented yet\n"));

        let mut args = vec!["--json", cmd];
        args.extend(rest);
        let r = run(&s.env, &args);
        assert_eq!(r.code, 1);
        let v = r.json();
        assert_eq!(v["status"], "error");
        assert_eq!(
            v["error"]["$type"],
            "dev.thmsn.standards.cli.command.error.not_implemented"
        );
        assert_eq!(v["error"]["exit_code"], 1);
        assert_eq!(r.err, "");
    }
}

#[test]
fn init_writes_the_four_files_then_never_clobbers() {
    let mut s = Scratch::new();
    let root = s.repo("my-tool");
    let sub = root.join("src");
    std::fs::create_dir(&sub).unwrap();
    s.env.cwd = sub; // runs from inside the repo, writes at the root

    let r = run(
        &s.env,
        &["--host", server(), "init", "--archetype", "cli-tool"],
    );
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    assert_eq!(
        r.out,
        "wrote .standards.conf\nwrote AGENTS.md\nwrote SHORTCUTS.md\nwrote README.md\n"
    );
    assert_eq!(
        tree(&root),
        [".standards.conf", "AGENTS.md", "README.md", "SHORTCUTS.md"]
    );

    let conf = StandardsConf::parse(&read(root.join(".standards.conf"))).unwrap();
    assert_eq!(conf.product, "my-tool");
    assert_eq!(conf.product_upper, "MY_TOOL");
    assert_eq!(conf.archetype.as_deref(), Some("cli-tool"));
    let agents = read(root.join("AGENTS.md"));
    assert!(
        agents.starts_with("Standards: archetype=cli-tool product=my-tool — rules via"),
        "{agents}"
    );
    assert!(!agents.contains("{{"));
    assert!(read(root.join("SHORTCUTS.md")).contains("**my-tool**"));
    let readme = read(root.join("README.md"));
    assert!(readme.contains("# my-tool"));
    assert!(
        readme.contains("library or CLI"),
        "library README for cli-tool"
    );

    // Edited files survive a rerun; a deleted one is backfilled from the recorded identity.
    std::fs::write(root.join("README.md"), "mine\n").unwrap();
    std::fs::remove_file(root.join("SHORTCUTS.md")).unwrap();
    let r = run(&s.env, &["--host", server(), "--json", "init"]);
    assert_eq!(r.code, 0);
    let v = r.json();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["archetype"], "cli-tool");
    assert_eq!(v["data"]["root"], root.display().to_string());
    let actions: Vec<(String, String)> = v["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                f["action"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expect = [
        (".standards.conf", "skipped"),
        ("AGENTS.md", "skipped"),
        ("SHORTCUTS.md", "wrote"),
        ("README.md", "skipped"),
    ];
    assert_eq!(actions, expect.map(|(p, a)| (p.to_owned(), a.to_owned())));
    assert_eq!(read(root.join("README.md")), "mine\n");
    assert!(read(root.join("SHORTCUTS.md")).contains("**my-tool**"));
}

#[test]
fn init_flags_and_verbose() {
    let mut s = Scratch::new();
    let root = s.repo("checkout");
    let r = run(
        &s.env,
        &[
            "--host",
            &format!("{}/", server()),
            "-v",
            "init",
            "--product",
            "vision",
            "--lib",
            "libvision",
            "--upper",
            "VIS",
        ],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.starts_with(&format!("server {} (commit ", server())),
        "{}",
        r.out
    );
    let conf = read(root.join(".standards.conf"));
    assert_eq!(
        conf,
        "# Product identity for the standards tooling.\nPRODUCT=vision\nPRODUCT_UPPER=VIS\n\
         LIB=libvision\nARCHETYPE=full-stack-product\n"
    );
    assert!(read(root.join("README.md")).contains("<img src=\"branding/brand/icon.svg\""));
}

#[test]
fn init_refuses_outside_a_repo_and_bad_input_without_writing() {
    let mut s = Scratch::new();
    let r = run(&s.env, &["--host", server(), "init"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("not in a git repo"), "{}", r.err);

    let root = s.repo("r");
    let r = run(&s.env, &["--host", server(), "init", "--archetype", "nope"]);
    assert_eq!(r.code, 2);
    assert!(
        r.err
            .contains("unknown archetype `nope`; one of: cli-tool,"),
        "{}",
        r.err
    );
    let r = run(&s.env, &["--host", server(), "init", "--product", "a b"]);
    assert_eq!(r.code, 2);
    std::fs::write(root.join(".standards.conf"), "not a conf\n").unwrap();
    let r = run(&s.env, &["--host", server(), "init"]);
    assert_eq!(r.code, 1);
    assert_eq!(tree(&root), [".standards.conf"]);
}

#[test]
fn unreachable_server_exits_three_and_writes_nothing() {
    let mut s = Scratch::new();
    let root = s.repo("r");
    let dead = dead_host();
    let r = run(&s.env, &["--host", &dead, "init"]);
    assert_eq!(r.code, 3, "{}", r.err);
    assert!(
        r.err.starts_with(&format!("error: can't reach {dead}")),
        "{}",
        r.err
    );
    assert_eq!(tree(&root), Vec::<String>::new());

    // STANDARDS_HOST is the fallback for --host.
    s.env.config.host = Some(dead.clone());
    let r = run(&s.env, &["--json", "new", "library", "x"]);
    assert_eq!(r.code, 3);
    assert_eq!(
        r.json()["error"]["$type"],
        "dev.thmsn.standards.cli.server.error.unreachable"
    );
    assert!(!s.env.cwd.join("x").exists());

    let r = run(&s.env, &["--host", "ftp://nope", "init"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("not an http(s) URL"));
}

#[test]
fn new_scaffolds_per_archetype() {
    let s = Scratch::new();
    let r = run(&s.env, &["--host", server(), "new", "cli-tool", "tool"]);
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    let root = s.env.cwd.join("tool");
    assert!(root.join(".git").exists(), "git init ran");
    assert_eq!(
        tree(&root),
        [
            ".standards.conf",
            "AGENTS.md",
            "README.md",
            "SHORTCUTS.md",
            "VERSIONING.md",
            "build.rs"
        ]
    );
    assert!(read(root.join("build.rs")).contains("APP_VERSION"));
    assert!(r.out.starts_with("git init "));

    let r = run(
        &s.env,
        &[
            "--host",
            server(),
            "--json",
            "new",
            "full-stack-product",
            "prod",
            "elsewhere/p",
        ],
    );
    assert_eq!(r.code, 0, "{}", r.out);
    let v = r.json();
    assert_eq!(v["data"]["git_init"], true);
    let root = s.env.cwd.join("elsewhere/p");
    assert!(tree(&root).contains(&"branding/build.py".to_owned()));
    assert!(read(root.join("branding/build.py")).contains("prod"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(root.join("branding/build.py"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }

    let r = run(
        &s.env,
        &["--host", server(), "new", "vendored-app", "v", "-v"],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.starts_with("server "));
    assert_eq!(tree(&s.env.cwd.join("v")).len(), 4);
}

#[test]
fn new_refuses_non_empty_dirs_and_unknown_archetypes() {
    let s = Scratch::new();
    let dir = s.env.cwd.join("taken");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("x"), "").unwrap();
    let r = run(&s.env, &["--host", server(), "new", "library", "taken"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("is not empty"), "{}", r.err);

    let r = run(&s.env, &["--host", server(), "new", "nope", "fresh"]);
    assert_eq!(r.code, 2);
    assert!(!s.env.cwd.join("fresh").exists());

    // An existing empty directory is fine.
    std::fs::create_dir(s.env.cwd.join("empty")).unwrap();
    let r = run(
        &s.env,
        &["--host", server(), "new", "library", "lib", "empty"],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(s.env.cwd.join("empty/VERSIONING.md").exists());
    let _ = s.path();
}

#[test]
fn client_reads_the_rest_mirror() {
    use standards_cli::client::{Client, ClientError, DEFAULT_TIMEOUT};
    let c = Client::new(server(), DEFAULT_TIMEOUT).unwrap();
    let templates = c.templates().unwrap();
    assert!(templates.iter().any(|t| t.path == "rust/build.rs"));
    let doc: serde_json::Value = c
        .get(&["api", "v1", "docs", "archetypes/cli-tool"], &[])
        .unwrap();
    assert_eq!(doc["id"], "archetypes/cli-tool");
    let e = c
        .render_template("nope.md", &std::collections::BTreeMap::new())
        .unwrap_err();
    assert!(matches!(e, ClientError::NotFound { .. }), "{e}");
    assert_eq!(e.key(), "dev.thmsn.standards.cli.server.error.not_found");
    let e = c
        .get::<serde_json::Value>(&["api", "v1", "search"], &[])
        .unwrap_err();
    assert!(matches!(e, ClientError::Status { status: 400, .. }), "{e}");
    let e = c.get::<Vec<u8>>(&["health"], &[]).unwrap_err();
    assert_eq!(e.key(), "dev.thmsn.standards.cli.server.error.bad_response");
}
