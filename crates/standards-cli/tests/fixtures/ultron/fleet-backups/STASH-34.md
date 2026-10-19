# STASH-34 — nightly restore drills (implementer journal)

Worktree: ~/git/stash/.claude/worktrees/agent-a910a2fe1e43d77ad
Branch: worktree-agent-a910a2fe1e43d77ad (rebased onto main 5b42f6c)

PLAN        — [x] done  [ ] not yet
- [x] 0. rebase onto main 5b42f6c; read PROGRAM/section/STASH-25,30-33 comments; code recon
- [x] 1. vision start comment with plan
- [x] 2. db: migration 0007_backup_drill.sql + `stash_db::backups` (BackupDrill row, record_backup_drill, latest_backup_drills, backup_drills(product, limit)) + tests
- [x] 3. storage: `reader::product_path(place, product) -> Result<PathBuf>` (pub; health-checked) for the drill's product_dir
- [x] 4. engine: `backups/drill.rs` — drill_one (resolve newest daily, ISIZE space check via injected probe, spawn_blocking stash_backup::drill on the exact set, record row), scratch wipe (only `drill-*` entries), slot/due logic
- [x] 5. engine: `backups/run.rs` — Settings (clock + free-space probe injectable), Handle (latest Report, nudge(product)), Report, audit pass (read_all + mirrors via spawn_blocking, latest drill states, audit), run loop with select!/shutdown
- [x] 6. engine tests (fixture via real stash_backup::once; rusqlite dev-dep): ok drill recorded; corrupt gz failed+reason; scratch empty both; reboot no re-drill; degraded skipped (record nothing); no-scratch skipped recorded; audit Report via callback + Handle; nudge drills now
- [x] 7. api: Config::backups_engine() settings; main.rs spawn beside janitor (clone Place before AppState), join on shutdown; callback no-op until STASH-35
- [x] 8. docs: DESIGN.md Application backups -> drills; engine lib.rs doc; server/README.md if needed; AGENTS.md status line
- [x] 9. gates: fmt, build, clippy -D warnings, test --workspace; openapi.json unchanged
- [x] 10. commit `engine, db, api: nightly restore drills (STASH-34)`; finishing vision comment

STATE       — DONE. Commit f959486; finishing comment posted on STASH-34.

TOUCHED     — server/db/migrations/0007_backup_drill.sql (new), server/db/src/backups.rs (new, 2 tests green), server/db/src/lib.rs (mod + doc); server/storage/src/backups/reader.rs (pub fn product_path + test asserts), server/storage/src/backups.rs (re-export); server/engine/src/backups.rs (mods + re-exports), backups/drill.rs (new), backups/run.rs (new), engine/Cargo.toml (+rustix fs, +rusqlite dev), Cargo.lock (+2 lines); engine/src/backups/run/tests.rs (new); api/src/config.rs, api/src/main.rs; DESIGN.md, README.md, server/README.md, engine/src/lib.rs. Commit f959486.

VERIFY      — from server/: cargo fmt --all -- --check; cargo build --workspace --all-targets;
              cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace;
              ./target/debug/stash-api openapi | diff - ../api/openapi.json
              last result (2026-10-13): fmt ok; build ok; clippy -D warnings ok; tests green (api 264, backup 125, db 54, engine 85, storage 149, thumbs 38); openapi byte-identical; one libsqlite3-sys (0.30.1)

DECISIONS (draft)
- degraded backups root: record NOTHING; night stays due, retried every 15 min. A drill that fails and finds the root degraded afterwards is discarded too.
- Err from stash_backup::drill (no daily set, gunzip, config) = failed with reason word.
- no-scratch = skipped recorded; counts as the night's drill (no retry storm).
- scratch wipe removes only `drill-*` entries (what stash_backup makes), never the dir or other files (misconfigured scratch == data_dir must not eat stash.db).
- free-space need = 2 * sum(ISIZE of .db.gz files in the set); mariadb .sql.gz not unpacked (LD-2) so not counted. ISIZE is mod 2^32 (known limit >4GiB).
- Handle lives in main for now; STASH-36 threads it into AppState.
