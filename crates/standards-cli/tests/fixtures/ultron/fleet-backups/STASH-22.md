# STASH-22 — backup crate: NAS primitives (sentinel, atomic publish, prune, daily-first)

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-aaa71d7890e7f0a24
Branch: worktree-agent-aaa71d7890e7f0a24 (reset --hard to main 07bf110 per ultron; workspace clippy gate now has NO allows)

PLAN
- [x] 0. ff worktree to 33ffd02; read sidecar.md, PROGRAM.md, B3.md, backups.md, vision-backup.sh, backup/src/*, document.rs, STASH-20 comments
- [x] 1. vision comment: start + PLAN
- [x] 2. new server/backup/src/atomic.rs: stage(<final>.partial, sync_all), commit(rename), sync_dir, write/copy helpers
- [x] 3. new server/backup/src/nas.rs: Target::open (sentinel, no-target, no writes), publish(at, &[Staged]) all-or-none with daily-first, Published (sealed), prune(&Published, &Retention) by name, non-fatal
- [x] 4. snapshot.rs: list_snapshots (reads effective config; empty if no db/), snapshot_sets(dir, dbs), SnapshotSel::resolve
- [x] 5. lib.rs: two `pub mod` lines + re-exports (minimal)
- [x] 6. behaviour tests per acceptance line (tempdirs, injected now)
- [x] 7. gates (VERIFY) + coverage sanity
- [x] 8. commit `backup: ...`
- [x] 9. vision finishing comment with evidence

STATE
- DONE. Finishing comment posted on STASH-22. Nothing half-applied.

TOUCHED
- server/backup/src/atomic.rs (new)
- server/backup/src/nas.rs (new)
- server/backup/src/snapshot.rs (list_snapshots, snapshot_sets, SnapshotSel::resolve, tests)
- server/backup/src/lib.rs (pub mod atomic, nas; pub use nas::{...})
- server/README.md (backup status cell)
- commit febb8c3 on worktree-agent-aaa71d7890e7f0a24 (parent d40e7a8)

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy -p stash-backup --all-targets -- -D warnings && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result (post-rebase on d40e7a8): fmt OK, build OK, clippy -p OK, clippy workspace (no allows) OK, test workspace all green (stash-backup 66). llvm-cov: atomic 98%, nas 97.8%, snapshot 99.3% lines
