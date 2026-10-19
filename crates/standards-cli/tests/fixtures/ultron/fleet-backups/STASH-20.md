# STASH-20 journal — backup crate: skeleton, config and the on-NAS contract types

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-ada0f7e8343b2c3e3
Branch: worktree-agent-ada0f7e8343b2c3e3

PLAN
- [x] 1. Read spec (sidecar.md, backups.md, PROGRAM.md), repo files, vision-backup.sh
- [x] 2. Comment start on STASH-20 with plan (comment ZXQTESG79WM)
- [x] 3. Workspace wiring: members, [workspace.dependencies] stash-backup + rusqlite =0.32.1 (bundled, backup) w/ comment, tarpaulin exclude, server/README layering + row, README.md workspace row
- [x] 4. server/backup/Cargo.toml + src/main.rs stub (usage, exit 2)
- [x] 5. src/error.rs: Reason enum (+Unknown serde(other)) + Subject + BackupError (plain data)
- [x] 6. src/config.rs
- [x] 7. src/snapshot.rs: Tier, SnapshotSel, SnapshotSet, parse_snapshot_name (real), list_snapshots (todo!)
- [x] 8. src/runs.rs: RunRecord (+ nested), read_runs (real), read_effective (real)
- [x] 9. src/verify.rs: Placement (real ctors), Pace, VerifyReport, DrillReport, verify/drill (todo!)
- [x] 9b. src/lib.rs tying modules together
- [x] 10. Tests compile + pass
- [x] 11. Gates: fmt, build, clippy, test, cargo tree -i libsqlite3-sys
- [x] 12. Commit `backup: ...`
- [x] 13. Finishing comment on STASH-20 with evidence + decisions + spec disagreements

STATE
- DONE. Commit 33ffd02; finishing comment posted on STASH-20. Open item for Ultron: 16 pre-existing clippy::assert_is_empty hits (clippy 1.99) in api/db/storage tests.

TOUCHED
- server/Cargo.toml, server/tarpaulin.toml, server/README.md, README.md
- server/backup/{Cargo.toml, src/main.rs, src/lib.rs, src/error.rs, src/config.rs, src/snapshot.rs, src/runs.rs, src/verify.rs}
- commit 33ffd02 "backup: the stash-backup crate skeleton, config and on-NAS contract"

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo tree -i libsqlite3-sys
- last result 2026-10-13: fmt/build/test green; tree = libsqlite3-sys v0.30.1 only; clippy green with -A clippy::assert_is_empty (pre-existing hits outside backup/)
