# STASH-25 — backup crate: verify, restore and drill library API

Branch: worktree-agent-ab6f974bad4cb5e77 (worktree /Users/charlesthomson/git/stash/.claude/worktrees/agent-ab6f974bad4cb5e77), rebased on main @ 15b21c8 (after ENOSPC stop; STASH-24/32/33 landed).

PLAN
- [x] 0. rebase worktree branch onto main (09ee9dc)
- [x] 1. read spec (sidecar.md, PROGRAM.md LDs, backups.md, vision-backup.sh restore), STASH-20..23 comments, crate source
- [x] 2. comment start (posted E5WJSB1MF8C) on STASH-25 with plan
- [x] 3. sqlite.rs open_read_only + mirror.rs Required::query uses it
- [x] 4. fill verify.rs `verify` (+ tests: FK fail, mirror sha mismatch/missing, pace)
- [x] 5. restore.rs: gunzip/unpack + `restore` (+ tests: corrupt => untouched, wal/shm aside, ownership, verify ok)
- [x] 6. `drill` (+ test: passes, product_dir byte-identical, scratch cleaned)
- [x] 7. gates: fmt, build, clippy -D warnings, test; coverage check
- [x] 8. commit (5b42f6c) `backup: ... (STASH-25)`
- [x] 9. finishing comment on STASH-25

STATE
Resumed after ENOSPC. WIP commit f909b86 'wip STASH-25' on top of 15b21c8 (to be amended into the final commit). gunzip moved to gz::decompress (gz.rs from STASH-24). step 7 (gates). All tests written; stash-backup 125 green; mutations (skip -wal move, skip precheck, no immutable) each go red. Implementation written for 4,5,6 and clippy-clean (no tests yet: src/restore/tests.rs is a placeholder). Design settled:
- sqlite.rs: add pub(crate) open_read_only(path) (immutable=1 iff WAL header and no -wal/-shm; else mode=ro) so verify never creates -wal/-shm beside a restored db (root-owned -shm would lock the service out). mirror.rs Required::query switched to it (1 line).
- verify.rs: verify -> verify_with(cfg, at, pace, &mut dyn Clock); check_dbs shared with restore precheck; collect all failures; one Throttle per sweep.
- restore.rs (new): pub(crate) gunzip/unpack(set -> dir); pub fn restore(cfg, which, dir, pace) -> Result<RestoreReport>; pub fn parse_selection(arg). work dir in cfg.state_dir; precheck = check_dbs + mirror::required on unpacked copies => Err, DIR untouched; stage partials in DIR, chown partials to old db owner (fail => remove partials, DIR untouched), move db/-wal/-shm to DIR/pre-restore-<stamp>/<rel>, commit; mirror::mirror(NAS files/<name> -> DIR mirror, expected rows) additive, chown new entries (tree diff); final verify.
- drill: in verify.rs, unique subdir under scratch, refuse scratch under product_dir, mariadb dumps skipped, remove subdir after.

TOUCHED
server/backup/src/sqlite.rs (open_read_only), mirror.rs (Required::query uses it), verify.rs (verify/verify_with/check_dbs/drill/drill_with), restore.rs (NEW: restore, parse_selection, unpack, gunzip, work_dir, refuse_inside), restore/tests.rs (placeholder), lib.rs (pub mod restore + re-exports), Cargo.toml (flate2). No commits yet.

VERIFY
cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
last result (5b42f6c): fmt clean, build ok, clippy -D warnings clean, test --workspace green (backup 125, api 263, storage 149, engine 75, db 52, thumbs 38). llvm-cov backup: crate 94.6% lines, restore.rs 76.7%, verify.rs 99.5%.
