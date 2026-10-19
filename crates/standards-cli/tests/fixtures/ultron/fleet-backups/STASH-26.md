# STASH-26 — stash-backup binary: loop, once, health, verify, restore

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-aa9614ba603ec7636 (branch worktree-agent-aa9614ba603ec7636), rebased on main 5b42f6c.

PLAN
- [x] 0. rebase on main (5b42f6c), write this journal
- [x] 1. read specs: backups.md, sidecar.md, PROGRAM.md LDs, STASH-20..25 comments, lib API (config, once, verify, restore)
- [x] 2. vision start comment with PLAN

DECISIONS (so far)
- loop runs once at boot then at next_run (vision-backup.sh does; local state file is in /tmp, lost on every
  watchtower recreate, so without a boot run health is unhealthy for up to ~50 min after each image roll)
- next_run strictly after now; loop re-sleeps if wall clock < target after wake (monotonic vs wall drift)
- signals: tokio::signal::unix directly (registered on creation, before `starting` line); libsignal's async fn
  registers only on first poll
- `--config PATH` global flag (default /etc/stash-backup/config.toml) — needed for process tests + host restores
- exit codes: 0 ok, 1 failed/config/unhealthy, 2 usage (never for health: docker reserves 2)
- tracing-subscriber fmt to stderr (stdout stays the raw LD-10 lines)
- [x] 3. backup/build.rs (RELEASE_VERSION, copy of api/build.rs) + Cargo.toml deps tokio, tracing-subscriber
- [x] 4. src/cli.rs: next_run, health, guarded run (catch_unwind), verify/restore/once command fns, arg parsing; lib.rs `pub mod cli;`
- [x] 5. main.rs thin: parse, dispatch; loop with current-thread tokio + signals (builds, clippy -p clean)
- [x] 6. unit tests (next_run table, health, panic wrapper) + tests/cli.rs process-level (once ok/config err, verify/restore codes, SIGTERM)
- [x] 7. gates: fmt, build, clippy -D warnings, test; README/docs touch if usage described
- [x] 8. commit c3be43e; vision finishing comment posted
- [x] 9. rebase (now 57af26f on b3348cb, README row merged) onto main b3348cb (STASH-27/42/43); README backup row: merge both descriptions
- [x] 10. restore REQUIRES DIR (missing => usage, exit 2); verify [DIR] stays optional; keep --bytes-per-sec flag
- [x] 11. four gates (green at 36ef80f)
- [x] 12. new commit 36ef80f; vision follow-up comment (SHA, gates, verify-DIR deviation)

STATE
- DONE (follow-ups too). Commits 57af26f (rebased c3be43e) + 36ef80f on b3348cb. Nothing half-applied.

TOUCHED
- server/backup/build.rs (new), server/backup/Cargo.toml (tokio, tracing-subscriber), server/backup/src/error.rs (pub(crate) quote_if_needed), src/cli.rs (new), src/cli/tests.rs (placeholder), src/main.rs (rewritten), src/lib.rs (pub mod cli), tests/cli.rs (new), server/README.md, Cargo.lock; commit c3be43e

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result (c3be43e): fmt clean, build ok, clippy -D warnings clean, test --workspace green (backup lib 134, tests/cli 6, api 263, storage 149, engine 75, db 52, thumbs 38)
