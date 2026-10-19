# STASH-43 — backup: time out a hung mariadb-dump
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a9a67e68dc944c09f (branch worktree-agent-a9a67e68dc944c09f, rebased onto main 00ea494)

PLAN
- [x] 0. Rebase onto main (00ea494); read STASH-27 comments, PROGRAM LDs
- [x] 1. Start comment on STASH-43 with this plan
- [x] 2. config.rs: top-level `mariadb_timeout: Duration` (duration_text, default 30m), Debug, validate (>0), defaults test + load test
- [x] 3. mariadb.rs: `dump(.., timeout, ..)`: spawn, drain stderr on a thread, poll try_wait to a deadline; on expiry kill + wait (reap), Err(mariadb-dump, "timed out after <duration>"); module doc. once.rs call site passes cfg.mariadb_timeout (one line, unavoidable)
- [x] 4. Tests: fake client `exec sleep 30` with a tiny timeout -> mariadb-dump, detail "timed out after", file removed, returns well before 30s; once-level: nothing published
- [x] 5. Gates from server/: fmt --check, build --all-targets, clippy -D warnings, test --workspace
- [x] 6. Commit `backup: time out a hung mariadb-dump (STASH-43)`
- [x] 7. Finish comment with SHA + evidence + decisions

STATE
- DONE. finish comment posted.

TOUCHED
- server/backup/src/config.rs, mariadb.rs, mariadb/tests.rs, once.rs (call site), once/tests.rs (one test)

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: all green at d6d9d4b (fmt clean, build+clippy Finished, tests api 263, backup 142+1 ignored, db 52, engine 75, storage 149, thumbs 38)
- commit d6d9d4b
