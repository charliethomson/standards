# STASH-35 journal (stash_backup_* metrics)

PLAN
- [x] 0. rebase worktree onto main (c13dd49)
- [x] 1. read metrics.rs (DupeMetrics, render), engine backups Report/Product/Audit/Drill, main.rs spawn_backups, STASH-33 comments
- [x] 2. comment start on STASH-35 with plan
- [x] 3. BackupMetrics in server/api/src/metrics.rs: state map updated from on_report(&Report); rendered in render
- [x] 3b. db: Db::last_ok_backup_drills() (newest ok per product) + test
- [x] 4. main.rs spawn_backups(&Metrics): seed_backups(&db) then run with on_report = metrics.backups
- [x] 5. tests: exposition (names/TYPE/labels, ok + failing product), counter once per drill across two reports, no [backups] => no series
- [x] 6. gates: fmt, build, clippy -D warnings, test --workspace; openapi unchanged
- [x] 7. docs touch (AGENTS/README/DESIGN if they list metrics), commit
- [x] 8. finishing comment on STASH-35

STATE
- step 3 (metrics.rs). Decisions: fail-only; removed product drops series; degraded/error-with-no-products keeps values; never-happened timestamps = 0; no [backups] = zero samples; duration absent until known

TOUCHED
- README.md, server/Cargo.lock, server/api/Cargo.toml, server/api/src/{metrics,main}.rs, server/db/src/backups.rs
- commit 5a8ba3b on worktree-agent-a085138bfb55372c7

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: all green 2026-10-13 (api 269, db 55, engine 85, backup 152+7, storage 148, thumbs 38); openapi byte-identical

LOG
- steps 3, 3b, 4 applied (uncommitted): api/Cargo.toml (+stash-backup dep), api/src/metrics.rs (BackupMetrics, Metrics::backups, seed_backups), db/src/backups.rs (last_ok_backup_drills + test), api/src/main.rs (spawn_backups(&Metrics)). Builds. Next: step 5 tests in metrics.rs.
- step 5 done: 5 tests in metrics::tests::backups all green; mutation (warn counts) turns 1 red. Sample exposition in scratchpad s35-sample.txt. Next: step 6 gates.
- steps 6,7 done; commit 5a8ba3b. Next: step 8 finishing comment.
- step 8 done: finishing comment posted. TASK COMPLETE.
