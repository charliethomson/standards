# STD-21 journal (standards doctor)
Worktree: /Users/charlesthomson/git/standards/.claude/worktrees/agent-af0cc2db6566b388e, branch std-21 (from origin/main 74be252)

PLAN
- [x] 1. Read specs, cli src, core, docs, libid shape (read-only)
- [ ] 2. env.rs: add `path: Option<OsString>` (PATH) additively; fix struct literals (env.rs tests, tests/harness)
- [ ] 3. doctor.rs + doctor/{mod parts}: Args, Check model, render (human/json), run loop (select, fix, re-check), quiet, ci, only, repo
- [ ] 4. user checks: mcp.registered, mcp.reachable, skills.synced, hook.session-start, cli.current, legacy.path-links
- [ ] 5. repo checks: repo.conf, repo.agents, legacy.submodule, legacy.skill-links, legacy.mcp-json, legacy.ci-override
- [ ] 6. tests/doctor.rs (+ unit tests): fake claude on temp PATH, temp HOME, temp git repos, real server in-process; update tests/cli.rs unimplemented test
- [ ] 7. coverage >=80 (cargo llvm-cov -p standards-cli --summary-only), clippy, fmt, workspace tests
- [ ] 8. acceptance: temp HOME + cp -R ~/git/libid to scratch, local server ephemeral port, fake claude; doctor / --fix / git status / doctor / --quiet
- [ ] 9. crates/README.md + CHANGELOG.md; commit; rebase origin/main; retest; push
- [ ] 10. vision finishing comment; delete target/

Decisions (local): serde_json preserve_order feature in cli; quiet skips repo scope? NO, includes it; non-ci repo with no .standards.conf and no legacy markers => repo scope skipped; claude child gets HOME=env.home, PATH=env.path.

STATE
step 2 starting

TOUCHED
(none)

VERIFY
cargo test --workspace; cargo llvm-cov -p standards-cli --summary-only; cargo clippy --workspace --all-targets -- -D warnings
last result: n/a
