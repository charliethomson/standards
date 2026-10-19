# STASH-42 — backup: read_runs returns its skipped-line count
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a9a67e68dc944c09f (same branch as STASH-43; done after the STASH-43 commit)

PLAN
- [x] 1. Start comment on STASH-42 with this plan
- [x] 2. runs.rs: add `read_runs_counted(product_dir, tail) -> Result<(Vec<RunRecord>, usize)>` (one read; same lenient rules); `read_runs` delegates. Export in lib.rs (one line).
- [x] 3. Test in runs.rs for the count (torn tail, malformed lines, blank lines not counted, counted over the whole file regardless of tail)
- [x] 4. storage reader.rs: `runs()` uses read_runs_counted; delete skipped_lines + its second read
- [x] 5. Gates from server/
- [x] 6. Commit `backup, storage: read_runs counts what it skips (STASH-42)`
- [x] 7. Finish comment

STATE
- DONE. finish comment posted.

TOUCHED
- server/backup/src/runs.rs, server/backup/src/lib.rs, server/storage/src/backups/reader.rs, DESIGN.md

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: all green at b3348cb (fmt clean, build+clippy Finished; tests api 263, backup 143+1 ignored, db 52, engine 75, storage 148, thumbs 38)
