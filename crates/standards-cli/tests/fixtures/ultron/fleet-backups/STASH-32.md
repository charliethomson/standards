# STASH-32 journal (implementer)

PLAN
- [x] 0. rebase worktree branch onto main (5a9d33a)
- [x] 1. read code: storage/backups.rs Place, MountedRoot (unfenced, sentinel/RootDegraded), backup crate (runs, effective, snapshot, layout)
- [x] 2. decide placement: stash_storage::backups (beside Place; storage is the only crate touching /nfs; storage gains stash-backup dep, no cycle). comment start on vision with PLAN
- [x] 3. implement reader module: list products, read_product (sentinel, effective, runs+skipped count, snapshots per tier w/ bytes, partials w/ age), mirror_summaries separately
- [x] 4. tests per acceptance line (fixture trees, real files, stash-backup writers)
- [x] 5. docs: DESIGN.md / server README if behaviour/layout described
- [x] 6. gates: fmt, build, clippy -D warnings, test --workspace
- [x] 7. commit; finishing vision comment

STATE
- DONE: committed 659ce4e; finishing comment posted on STASH-32. Nothing half-applied.

TOUCHED
- server/storage/Cargo.toml (+stash-backup dep)
- server/storage/src/backups.rs (pub mod reader + re-exports, doc line)
- server/storage/src/backups/reader.rs (new)
- DESIGN.md (### The reader), server/README.md, server/Cargo.lock
- commit 659ce4e storage: the backups reader over the NAS layout (STASH-32)

VERIFY
- from server/: cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result (after docs, before commit): all green; storage 149, api 263, backup 66, engine 53, db 52, thumbs 38; llvm-cov reader.rs 98.65% lines
