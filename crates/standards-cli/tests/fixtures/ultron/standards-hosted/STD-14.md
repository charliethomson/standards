# STD-14 journal (worktree .claude/worktrees/agent-a355127bb2f390957, branch std-14 off origin/main fb1764d)

PLAN
- [x] 1. checkout std-14 from origin/main; write this journal
- [x] 2. read core src/tests, docs (service-arch, rust-conv, testing, build-info), warden web.rs
- [x] 3. vision: comment start with plan
- [x] 4. standards-db: rust-embed of repo root w/ include/exclude per L5, is_served guard, commit from libbuildinfo/RELEASE_COMMIT, OnceLock<Arc<Bundle>> loader; allowlist test
- [x] 5. standards-engine: Engine over Arc<Bundle>: search, checklist, doc/section, templates, skills, manifest; thiserror errors
- [x] 6. tests over real content; llvm-cov >=80% each; clippy -D warnings
- [x] 7. crates/README.md, workspace members; commit, rebase, rerun tests, push
- [x] 8. vision finishing comment

STATE
- DONE. 5be470e pushed to origin/main; finishing comment posted. Vision state not moved.

TOUCHED
- Cargo.toml, Cargo.lock, CHANGELOG.md, crates/README.md, crates/standards-db/**, crates/standards-engine/**; commit 5be470e (rebased from d8809ca)

VERIFY
- cargo test --workspace; cargo llvm-cov -p standards-db -p standards-engine --summary-only; cargo clippy --workspace --all-targets -- -D warnings
- last result (5be470e): all workspace tests pass; llvm-cov db 100% lines, engine 100% lines; clippy clean
- steps 4-5 done: crates build, tests pass, clippy clean (uncommitted). Next: llvm-cov, README, commit.
- committed d8809ca, rebased to 5be470e over STD-25 commits; rerunning tests
