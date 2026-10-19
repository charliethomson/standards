# STASH-7 — M4 thumbnails and preview
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a5b2fbe21d8fb5ad9
Branch: worktree-agent-a5b2fbe21d8fb5ad9 (do not push/merge)

PLAN
- [x] 0. read context, write this journal, vision comment + in_progress
- [x] 1. storage: `open_exact(&FileStat)` -> std File, fstat-verified dev/ino (TOCTOU guard) + tests
- [x] 2. thumbs crate: Geometry (160/320/640), Kind by guessed type, Refusal enum w/ reason(),
        image decode w/ image::Limits (max_alloc = MAX_DECODED_PIXELS*4), BufReader source,
        source-bytes cap on stat size (images only), EXIF orientation, alpha flatten, JPEG out;
        ffmpeg frame grab (stdin=verified fd, file:/dev/stdin, protocol_whitelist file,
        format_whitelist, -threads 1, RLIMIT_AS on linux via pre_exec, kill_on_drop + explicit
        kill + reap on timeout, stdout capped); semaphore = available_parallelism; spawn_blocking;
        cache dir (sharded key[..2]/key.jpg, temp+rename, touch mtime on hit, `.none` negative
        marker for deterministic refusals); `Thumbs::open(dir)` degrades to disabled; tests per cap
- [x] 3. engine: trim.rs — periodic LRU by mtime to cap (low watermark 90%), stale .tmp sweep, tests
- [x] 4. api: config (thumb_trim_interval_secs), state field, routes/thumb.rs
        `GET /api/v1/roots/{root}/thumb?path&size` getThumbnail; 304/415/503; metrics families;
        route_name + exact op set + READ_ONLY; main.rs boot (create /data/thumbs, spawn trim);
        tests incl. container-config /data test, cookie auth, degraded, grant 404, traversal
- [x] 5. regenerate api/openapi.json; web codegen; Listing thumbnails + Preview lightbox
- [x] 6. docs: README, AGENTS.md Status, DESIGN.md (milestones, thumbnails route form)
- [x] 7. verify: fmt, clippy, test, contract idempotent, web codegen:check/typecheck/build,
        tarpaulin >=80, docker build (if possible)
- [x] 8. commit; vision finishing comment

STATE
- steps 1+2 DONE (storage open_exact; thumbs crate lib/still/cache/video, 32 tests green, clippy clean).
  Thumbs API: Thumbs::open(Settings)->never fails; is_available; cache_dir; cached(key)->Option<Cached>;
  generate(std File, len, Kind, Geometry)->Result<Vec<u8>,Refusal>; store(key,&outcome); etag(key).
  cache layout: <dir>/<key[..2]>/<key>.jpg|.none, temps ".tmp-*" (cache::TEMP_PREFIX).
- step 3 DONE (engine/src/trim.rs + `pub mod trim` in engine lib.rs).
- step 4 DONE: metrics (3 thumb families), config (thumb_trim_interval_secs, Config::thumbs()),
  state (Parts.thumbs), routes/thumb.rs (ThumbApi getThumbnail, 16 tests), app.rs (service tuple,
  route_name, op set, READ_ONLY), main.rs (boot + trim spawn), testkit (app_without_thumbs),
  api dev-dep image. api 200 tests green, clippy/fmt clean.
- step 5 DONE: openapi regenerated; web schema.d.ts regenerated; client.ts thumbUrl; lib/media.ts;
  components/Preview.tsx; Listing thumbs + w-full name col fix; App preview state. typecheck+build ok;
  eyeballed against a node mock (scratchpad/m4/mock.mjs) — listing thumbs, fallback glyph, lightbox, video, keys ok.
- step 6 DONE: README (Thumbnails section, metrics, gaps), AGENTS.md (Status, new thumbnail section,
  metrics override), DESIGN.md (route form, milestone note), server/README, api/README, apps/web/README.
- step 7 DONE: fmt ok, clippy ok, tests 351 green, tarpaulin 93.43%, contract no drift, web codegen:check/typecheck/build ok. docker: daemon not running, not built.
- committed 00b5839; finishing vision comment posted. TASK COMPLETE (issue left in_progress).
- NOTE: scratchpad is SHARED with the M3 agent (it overwrote start.md) — use scratchpad/m4/ only.
- NOTE: docker daemon (OrbStack) not running; linux-only rlimit code untested locally (test added, cfg linux).

TOUCHED
- server/storage/src/lib.rs (open_exact + 2 tests)
- server/thumbs/{Cargo.toml, src/lib.rs, src/still.rs, src/cache.rs, src/video.rs}

VERIFY
- cd server && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
- cargo run -q --bin stash-api -- openapi > ../api/openapi.json && git diff --exit-code -- ../api/openapi.json
- cd apps/web && npm ci && npm run codegen:check && npm run typecheck && npm run build
- cd server && cargo tarpaulin --engine llvm
- last result: all green at 00b5839 (tarpaulin 93.43%)

REBASE (coordinator request)
- [ ] 9. rebase 00b5839 onto main (M3 = 331dfb5). Coordinator started `git rebase main`; stopped with
        conflicts. Resolve keeping both sides; regen openapi + schema.d.ts; engine doc all four jobs real;
        AGENTS/README/DESIGN both M3+M4 implemented; rebase --continue; full verify; docker retry.
- [x] 9. REBASE DONE: new SHA 4285b06 directly on 331dfb5. Resolved 14 files keeping both sides; openapi.json +
        schema.d.ts taken from HEAD then regenerated. Verify: fmt ok, clippy ok, tests 409 green
        (api 214, db 44, engine 33, storage 86, thumbs 32), tarpaulin 93.09%, contract no drift on 2nd regen,
        codegen:check exit 0, typecheck+build ok. docker: `orb start` brought daemon up; `docker build --ssh default .`
        failed at cargo fetch of private auth.git — local ssh agent has no identities (environmental). Not pushed.
