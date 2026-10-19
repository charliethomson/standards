# STASH-24 — backup crate: `once` orchestration, runs.jsonl and the log line

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a0cf10a25d2059556
Branch: worktree-agent-a0cf10a25d2059556 (rebased onto main 09ee9dc)

PLAN
- [x] 0. rebase onto main 09ee9dc; read PROGRAM.md, sidecar.md, backups.md, vision-backup.sh, STASH-20..23 comments
- [x] 1. vision comment: start + PLAN (H0N4B7RSRET)
- [x] 2. read server/backup/src/* (config, error, runs, nas, atomic, sqlite, mirror, snapshot)
- [x] 3. flate2 dep + src/gz.rs (compress helper, `gzip` reason)
- [x] 4. src/once.rs: Once{now clock, out writer, trigger, version}, run order sentinel→work→sqlite→files→require_files→[mariadb seam]→publish→prune→runs.jsonl→effective config→state→last-success→final line; RunResult for exit code; append_run in runs.rs (O_APPEND single write_all)
- [x] 5. RequireFilesRun: optional `mismatched` field (lenient)
- [x] 6. behaviour tests (src/once/tests.rs) per acceptance line
- [x] 7. gates (VERIFY), server/README.md backup cell
- [x] 8. commit (7547631) `backup: ... (STASH-24)`
- [x] 9. vision finishing comment with evidence (posted)

STATE
- DONE. Commit 7547631; finishing comment posted. Nothing half-applied.

TOUCHED
- server/Cargo.toml (flate2 ws dep), backup/Cargo.toml (flate2, regex dev), backup/src/gz.rs (new), runs.rs, mirror.rs, mirror/tests.rs, backup/src/once.rs (new), backup/src/once/tests.rs (new), lib.rs

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result (7547631): fmt OK, build OK, clippy workspace -D warnings OK, test workspace green (stash-backup 105). llvm-cov: once.rs 93.9% lines, gz.rs 100%, crate 96.0%
