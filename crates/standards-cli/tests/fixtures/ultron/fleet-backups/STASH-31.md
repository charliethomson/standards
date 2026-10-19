# STASH-31 — dev.thmsn.stash.backups grant

PLAN        — [x] done  [ ] not yet
- [x] rebase worktree branch onto main 07bf110
- [x] vision start comment with plan
- [x] auth.rs: `Backups` grant variant + doc table row (not in from_leaf)
- [x] storage: `stash_storage::backups::Place` {unfenced root, path}
- [x] state.rs: Parts.backups + AppState.backups field + `backups_for(user)` (enabled && Backups grant && root_for)
- [x] main.rs: mount_roots -> Mounted struct; fence_backups returns Option<Place>
- [x] testkit: Harness.state; Place when backups=true; fake tokens archivist/backups-only/backups-reader/ex-archivist
- [x] tests
- [x] contract check — byte-identical
- [x] gates
- [x] commit 58b1844
- [x] finishing vision comment

STATE       — DONE
TOUCHED     — AGENTS.md, DESIGN.md, server/api/src/{app,auth,config,main,state,testkit}.rs, server/storage/src/{backups.rs(new),lib.rs}; commit 58b1844 on worktree-agent-a4c789c5d483d80b6
VERIFY      — from server/: cargo fmt --all --check; cargo build --workspace --all-targets; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace — all green (api 263, storage 132, engine 53, db 52, thumbs 38, backup 33). One pre-existing flake seen once: thumbs video::tests::a_hung_ffmpeg_is_killed_at_the_deadline_and_reaped (pidfile read unwrap, video.rs:302), passed on 4 reruns.
