# STASH-23 — backup crate: additive file mirrors and require_files

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a2f3951eed36f3f18
Branch: worktree-agent-a2f3951eed36f3f18 (rebased onto main 5a9d33a)

PLAN
- [x] 0. rebase onto main 5a9d33a; read sidecar.md, PROGRAM.md LDs, backups.md steps 2-3, vision-backup.sh, backup/src/*, STASH-20..22 comments
- [x] 1. vision comment: start + PLAN
- [x] 2. deps: sha2 (0.10, already in Cargo.lock) + hex into stash-backup
- [x] 3. new server/backup/src/path.rs: safe_join (lexical, resolve_lexical shape) + within (canonicalize, re-check prefix) -> unsafe-path
- [x] 4. new server/backup/src/mirror.rs: query rows from snapshot (require-files-query), Expected map, additive recursive mirror (atomic stage/commit, sha check before rename LD-7), presence check (require-files-missing count=N), verify-side hash check with Pace + Clock (LD-11), run() tying step 2+3 -> FilesRun/RequireFilesRun
- [x] 5. lib.rs: pub mod path; pub mod mirror; (only)
- [x] 6. behaviour tests per acceptance line
- [x] 7. gates (VERIFY); server/README.md backup cell
- [x] 8. commit `backup: additive file mirrors and require_files (STASH-23)`
- [x] 9. vision finishing comment with evidence

STATE
- DONE. Finishing comment posted on STASH-23. Nothing half-applied.

TOUCHED
- server/Cargo.toml (sha2 workspace dep), server/backup/Cargo.toml (sha2, hex)
- server/backup/src/path.rs (new)
- server/backup/src/mirror.rs, server/backup/src/mirror/tests.rs (new)
- server/backup/src/lib.rs (pub mod mirror; pub mod path;)
- server/README.md (backup cell), server/Cargo.lock
- commit 09ee9dc on worktree-agent-a2f3951eed36f3f18 (parent 5a9d33a)

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: fmt OK, build OK, clippy workspace -D warnings OK, test workspace green (stash-backup 90). llvm-cov: mirror.rs 94.4% lines, path.rs 95.8%, crate 96.1%
