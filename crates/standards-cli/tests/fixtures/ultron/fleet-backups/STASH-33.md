# STASH-33 — stash: backup audits (implementer journal)

PLAN
- [x] 1. Rebase worktree branch onto main (f4b823a)
- [x] 2. Read reader (storage/src/backups/reader.rs), stash-backup contract types (RunRecord, EffectiveConfig), engine layout
- [x] 3. Comment start on STASH-33 with plan
- [x] 4. Write server/engine/src/backups.rs + backups/audit.rs: CheckKind enum (fixed names), Status, Check, DrillState/DrillOutcome, Audit{checks,status}, audit(&ProductView, now, Option<&DrillState>)
- [x] 5. Table tests per check + rollup + no runs + unreadable parts
- [x] 6. Docs: DESIGN.md "Application backups" audits subsection, server README engine row if needed
- [x] 7. Gates from server/: fmt, build, clippy -D warnings, test
- [x] 8. Commit `engine: backup audits (STASH-33)`
- [x] 9. Finishing comment on STASH-33

STATE
- DONE. Commit 6f1c614; finishing comment posted on STASH-33.

TOUCHED
- server/engine/Cargo.toml (stash-backup dep), server/engine/src/lib.rs (pub mod backups), new server/engine/src/backups.rs, new server/engine/src/backups/audit.rs, DESIGN.md (The audits), server/README.md (engine row), engine/src/lib.rs doc

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: fmt OK, build OK, clippy OK, tests green (api 263, backup 90, db 52, engine 75 incl 22 audit, storage 149, thumbs 38); llvm-cov audit.rs 99.88% lines
