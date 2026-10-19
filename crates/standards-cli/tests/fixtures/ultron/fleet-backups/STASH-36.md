# STASH-36 journal — stash: backups API and contract
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a849e3df1ddff03f3
Branch: worktree-agent-a849e3df1ddff03f3 (fast-forwarded to main c13dd49)

PLAN
- [x] 0. rebase on main (ff to c13dd49); read brief, section.md, LDs, STASH-31/34 comments
- [x] 1. vision start comment with plan
- [x] 2. read code: routes.rs download_file, app.rs route_name + contract tests, state.rs backups_for, main.rs spawn_backups, engine backups Report/Handle, storage backups reader, testkit
- [x] 3. extract serve(stat,&Request)->Download in routes.rs; download_file uses it
- [x] 4. Handle into AppState (main.rs run() creates Handle, passes to spawn_backups + AppState); accessor gated through backups_for; remove cfg_attr expect(dead_code)
- [x] 5. routes/backups.rs: Tag::Backups + 5 ops; register in app.rs; route_name entries; exact operationId set test
- [x] 6. tests (testkit harness w/ backups fixture): per-route 404s, 206/304, outside db/ 404, unknown product 404, drill no-Origin 403
- [x] 7. regen api/openapi.json; regen web client per web.ci.yml; apps/web typecheck/build
- [x] 8. docs: README/DESIGN/server README rows as needed
- [x] 9. gates: fmt, build, clippy -D warnings, test --workspace
- [x] 10. commit; finishing vision comment

STATE
- step 6 done: 15 tests in server/api/src/routes/backups/tests.rs all pass (cargo test -p stash-api backups: 35 passed). testkit: ARCHIVIST account, backups_loop field. api dev-dep rusqlite. clippy+fmt clean (testkit: backups_tree() helper extracted for too_many_lines; tests: eventually() returns bool). step 7 done: openapi regenerated (api/ + apple copy), web schema.d.ts regenerated, typecheck+build ok. step 8 docs done (README.md, server/README.md, DESIGN.md "### The API"). step 9 gates green (fmt, build, clippy -D warnings, test --workspace: api 279, backup 152+7, db 54, engine 85, storage 148, thumbs 38; contract byte-identical; one libsqlite3-sys 0.30.1). step 10 done: commit eb268e9; finishing vision comment posted. TASK COMPLETE.
- DESIGN NOTES (decided): types prefixed Backup* (SnapshotView/Place already exist in contract).
  state::Backups{place,handle}; backups_for returns &Backups. Parts.backups: Option<state::Backups>.
  list = latest Report when present & healthy, else fresh read_all+audit (mirrors None, drill from db).
  get = fresh read_product (+raw config.toml, no symlink follow) ; audit from report if product in it else fresh; mirrors from report (mirrors_as_of).
  runs = read_product with tail=usize::MAX, filter before, newest first, limit (default 50, max 500), skipped count.
  snapshot = product_path (spawn_blocking) + reject '..' + first seg 'db' + stat on unfenced root + stat.real under <product_dir>/db ; info log user+product.
  drill = product_path check (404) then nudge, 202. api gets stash-backup dep for RunRecord types.

TOUCHED
- server/api/src/routes.rs, server/api/src/routes/download.rs (serve + stream moved; builds)
- server/api/src/state.rs (state::Backups{place,handle}; backups_for -> &Backups; expect(dead_code) removed), testkit.rs (backups_loop field), main.rs (Handle created in run(), spawn_backups takes it), api/Cargo.toml (+stash-backup), api/src/routes/backups.rs (new), app.rs

VERIFY
- from server/: cargo fmt --all -- --check; cargo build --workspace --all-targets; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
- cargo run -q -p stash-api -- openapi > scratch/s36-openapi.json && diff with api/openapi.json
- web.ci.yml client drift + typecheck/build

COMMITS
- eb268e9 api, web: the backups API and contract (STASH-36)
