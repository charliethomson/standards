# STASH-21 — backup crate: SQLite snapshot primitive

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a4f6fde87e2fca440
Branch: worktree-agent-a4f6fde87e2fca440 (ff'd to main 07bf110; Ultron: workspace clippy gate is now WITHOUT -A assert_is_empty; rebase onto main before final commit)

PLAN
- [x] 0. ff worktree to 33ffd02; write this journal
- [x] 1. read sidecar.md, PROGRAM.md, backups.md, vision-backup.sh, server/backup/src/*, STASH-20 comments
- [x] 2. vision comment (VEQ4CTWDP7G): start + PLAN
- [x] 3. new module server/backup/src/sqlite.rs: header journal-mode, fingerprint, open (immutable vs ro+busy), Backup step(-1), integrity/fk/counts, result type; gzip_file helper only if needed; minimal lib.rs edits; Reason extensions in error.rs if missing
- [x] 4. behaviour tests (tempdirs, real sqlite) per acceptance line
- [x] 5. gates: fmt, build, clippy -p stash-backup, clippy workspace -A assert_is_empty, test workspace, cargo tree -i libsqlite3-sys
- [x] 6. commit (d40e7a8) `backup: ...`
- [x] 7. vision finishing comment with evidence (posted)

STATE
- DONE. Nothing half-applied.

TOUCHED
- server/backup/src/sqlite.rs (new), server/backup/src/sqlite/tests.rs (new), server/backup/src/lib.rs (+pub mod sqlite), server/README.md (backup row status). Commit d40e7a8 on worktree-agent-a4f6fde87e2fca440 (on main 07bf110).

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy -p stash-backup --all-targets -- -D warnings && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo tree -i libsqlite3-sys
- last result (07bf110 + d40e7a8): fmt OK, build OK, clippy crate+workspace (no allows) clean, test workspace all green (backup 44), libsqlite3-sys v0.30.1 only
