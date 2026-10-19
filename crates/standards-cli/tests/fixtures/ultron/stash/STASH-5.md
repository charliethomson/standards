# STASH-5 — M2: writes, resumable uploads and mutations

## PLAN
- [x] 0. Baseline: confirm cargo build/test works locally (protoc, private ssh deps)
- [x] 1. `storage`: path::resolve_parent (create-destination safety) + upload.rs
      (stage/append/finish-rename/abandon) + mutate (mkdir/remove/copy/rename)
- [x] 2. `db`: 0003 migration (fix 0001's entry/entry_fts rowid bug so write-through
      can maintain FTS) + upload.rs (CAS on received) + index.rs (write-through)
- [x] 3. `api`: origin.rs guard (Origin/Referer, cookie-credential only) + tests
- [x] 4. `api`: routes/uploads.rs (POST/PATCH/GET/POST finish/DELETE)
- [x] 5. `api`: routes/mutate.rs (mkdir, delete, move, copy) + cross-root refusal
- [x] 6. route_name table + metrics; contract regen; no-mutating-GET spec test;
      every-mutation-is-guarded spec test
- [x] 7. janitor in stash-engine + spawn from main.rs; config upload knobs
- [x] 8. app.rs container-config tests: staging path under real temp tree w/ real
      sentinel, real rename(2) across finish, degraded-root refusal, app_with_web
- [x] 9. interrupted-then-resumed large-ish upload test
- [x] 10. web: codegen, drag-to-upload w/ per-file progress, mkdir/delete/rename
- [x] 11. AGENTS.md + README.md (writes no longer a gap)
- [x] 12. gates: fmt, clippy, test, contract drift, npm codegen:check/typecheck/build,
      tarpaulin >= 80
- [x] 13. /thmsn-standards-review, pull --rebase, re-run gates, commit + push
- [ ] 14. vision comment with evidence

## STATE
Done and pushed as e8665d7 on main. Only step 14 (the vision comment) remains.

## TOUCHED
commit e8665d7 "api, storage, db, web: writes — resumable uploads and mutations"
41 files: core/src/error.rs; api/src/{app,auth,config,error,main,origin,principal,routes,
sessions,session_cookie,state,testkit,uploads}.rs; api/src/routes/{mutate,session,uploads}.rs;
storage/src/{lib,mutate,path,upload}.rs; db/src/{lib,index,upload}.rs;
db/migrations/0003_write_through.sql; engine/{Cargo.toml,src/lib.rs,src/janitor.rs};
apps/web/src/{App.tsx,api/{client-adjacent: keys.ts,mutations.ts,upload.ts,schema.d.ts},
components/{Listing,Uploads}.tsx}; apps/web/vite.config.ts; api/openapi.json;
AGENTS.md; README.md; DESIGN.md; server/Cargo.lock

## VERIFY — all run on the committed tree, all green
- `cargo fmt --all -- --check`                               -> no output
- `cargo build --workspace --all-targets`                    -> Finished
- `cargo clippy --workspace --all-targets -- -D warnings`    -> Finished, no diagnostics
- `cargo test --workspace`                                   -> 296 pass, 0 fail
  (api 184, db 27, engine 7, storage 77, thumbs 1)
- contract drift: regenerate api/openapi.json then diff --exit-code -> exit 0
- `npm run codegen:check` / `typecheck` / `build`             -> all clean
- `cargo tarpaulin --engine llvm --out Xml`                   -> 93.34% (floor 80)

## NOT VERIFIED
The 8GB-over-wifi resume. No NFS mount and no real auth service locally. The closest
available is in place: an interrupted-and-resumed upload over HTTP compared byte for byte,
the staging path and a real rename(2) (asserted by inode) against a real temp tree with a
real sentinel, and the degraded-root refusal.
