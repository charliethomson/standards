# STD-20 journal (standards cli skeleton: init, new, self-update)

PLAN
- [x] 0. setup: worktree on std-20 from origin/main (5fcd55e)
- [x] 1. read inputs (bin/standards, templates, docs, core, api rest, openapi, precedent cli)
- [x] 2. vision start comment with PLAN
- [x] 3. scaffold crates/standards-cli: Cargo.toml, main/lib, clap surface, exit codes, output (--json)
- [x] 4. client.rs (REST, timeouts, typed errors, unreachable->3) + cache.rs (manifest cache, XDG_CACHE_HOME, hourly)
- [x] 5. init + new (scaffold set per archetype, documented)
- [x] 6. self-update (registry API per docs/registry-publishing.md, atomic replace, --check)
- [x] 7. doctor/janitor/ultron stubs (own modules, TODO STD-21/22/23)
- [x] 8. tests: in-process standards-server on ephemeral port; fake registry; coverage >=80, clippy
- [x] 9. crates/README.md + CHANGELOG.md
- [x] 10. live diff vs bin/standards install; L32 registry install finding
- [x] 11. commit, rebase, retest, push; vision finish comment

STATE
- DONE: landed 74be252 on origin/main; vision finish comment posted; target/ removed. (older notes below are history.)
  ~/git/standards/.claude/worktrees/agent-ab2dedb6bf4487af0, branch std-20 (from origin/main 5fcd55e).
  cli code + tests done and green (24 unit + 9 cli + 7 self_update), clippy -p standards-cli
  --all-targets clean, cargo fmt done, llvm-cov 89.25% lines. Local server stopped.
- NEXT step 10: the sandbox refuses running bin/standards (bash/git in a scratch repo), so repo A
  was REPRODUCED by hand with bin/standards' exact sed pipelines at
  <scratchpad>/live/a/demo (product demo, archetype cli-tool; 4 files written; no .git dir yet).
  <scratchpad>/live/b/demo is git-inited and empty. To do: start
  `STANDARDS_BIND=127.0.0.1:8137 target/debug/standards-server`, run
  `target/debug/standards --host http://127.0.0.1:8137 init --product demo --archetype cli-tool`
  with cwd b/demo, `diff -r a/demo b/demo -x .git`. Expected only diff: .standards.conf comment
  line (core to_conf_string "# Product identity for the standards tooling." vs bin's
  "(genericizer + lint)").
- Then step 9 (crates/README.md: standards-cli row, drop "joins the workspace when it is built",
  Status; CHANGELOG Added entry), cargo test --workspace, clippy --workspace, commit, rebase,
  push, rm -rf target, vision finish comment.
- L32 finding: registry installer is #!/bin/sh POSIX (CI checks dash), so `| sh` works; but
  resolve/download need auth (REGISTRY_TOKEN or the installer's device sign-in), contrary to
  registry-publishing.md "reads are public". self-update sends REGISTRY_TOKEN as Bearer; it does
  not do the device flow or read the installer's stored refresh token (deferred).

TOUCHED
- Cargo.toml (member, workspace deps clap/reqwest/flate2/tar/tempfile, standards-api path dep),
  Cargo.lock, crates/standards-cli/** (new). No commits.

VERIFY
- cargo test --workspace; cargo llvm-cov -p standards-cli --summary-only; cargo clippy --workspace --all-targets -- -D warnings
- last: cargo test -p standards-cli green; llvm-cov 89.25% lines; clippy -p standards-cli clean
