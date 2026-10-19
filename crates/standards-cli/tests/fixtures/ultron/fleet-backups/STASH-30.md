# STASH-30 journal — [backups] config and the fence

PLAN
- [x] 1. Read code (config.rs, main.rs, state.rs, storage lib/path/walk, dupes, DESIGN, README, deploy toml)
- [x] 2. Post start comment on STASH-30 with plan
- [x] 3. config.rs: [backups] block + 4 scalars (+ tests)
- [x] 4. storage: MountedRoot.fenced, resolve/resolve_for_create NotFound inside fence, list + walk::scan skip, unfenced() (+ tests)
- [x] 5. main.rs: boot validation (unknown root, overlap with collections, read-grant root) + set fenced in mount_roots (+ tests)
- [x] 6. Route-level tests: listing omits, entries/download/thumb/delete/move/copy-into 404; no [backups] => unchanged
- [x] 7. Verify crawl sweep removes index rows under fence; digest rows too (test)
- [x] 8. DESIGN.md paragraph; README / deploy stash.toml decision
- [x] 9. Gates: fmt, build, clippy, test; commit
- [x] 10. Finish comment on STASH-30

STATE
- DONE. Commits f1051ce (clippy 1.99 lint fixes) and 18b5fd6 (feature) on worktree-agent-ab949d1ac172acb36; finishing comment posted on STASH-30. Nothing half-applied.

TOUCHED
- server/storage/src/{lib,path,walk,mutate,trash,upload,collections}.rs (fence, MountedRoot::{fence,fenced,unfenced,is_fenced,refuse_fenced,resolve_for_create,ensure_within}; path::ensure_within returns PathBuf)
- server/api/src/uploads.rs (root.resolve_for_create)
- server/api/src/{config,main,testkit,routes}.rs, server/api/src/routes/fence_tests.rs (new), server/engine/src/crawl.rs (test), deploy/compose/stash.toml
- DESIGN.md, README.md
- commits: f1051ce, 18b5fd6

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result (2026-10-13): fmt OK, build OK, clippy OK (after fixing 16 pre-existing assert_is_empty hits from rustc/clippy 1.99), test OK (api 257, storage 131, engine 53, db 52, thumbs 38); openapi diff unchanged
