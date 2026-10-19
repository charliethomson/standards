# STD-15 standards-api skeleton

PLAN
- [x] 1. Sync worktree to origin/main (branch std-15); read docs + precedents (warden/vision api) + engine/core/db src
- [x] 2. Vision start comment with PLAN
- [x] 3. Scaffold crates/standards-api (Cargo.toml, workspace member, lib.rs app builder w/ STD-16/17 mount markers, main.rs, config.rs)
- [x] 4. REST routes (/api/v1/*), errors -> 404 problem bodies, caching headers, /manifest.json, /t/{path}, /install, /health, /version, /api/metrics, /api/docs, /openapi.json
- [x] 5. openapi subcommand + committed spec + drift test
- [x] 6. Tests via TestClient; llvm-cov >=80; clippy clean; fmt
- [x] 7. crates/README.md + CHANGELOG.md
- [x] 8. Commit, rebase, test, push origin main; live curl run
- [x] 9. Vision finish comment with evidence

STATE
DONE. landed 5fcd55e on origin/main; finish comment posted on STD-15.

TOUCHED
Cargo.toml, Cargo.lock, CHANGELOG.md, crates/README.md, crates/standards-api/**, api/openapi.json; commit 5fcd55e (pushed)

VERIFY
cargo test --workspace; cargo llvm-cov -p standards-api --summary-only; cargo clippy --workspace --all-targets -- -D warnings; drift test; live curl

## progress log
- step 3/4 written (uncommitted): Cargo.toml (workspace deps+member), crates/standards-api/{Cargo.toml, src/{lib,main,config,metrics,request,error,dto,rest,state,caching,paths,mcp,web,tests}.rs}. Next: cargo build, fix, generate api/openapi.json.
- step 4/5 done: builds, 22 tests pass, clippy clean, api/openapi.json generated. Next: coverage.
- coverage 93.98% lines excl main (86.12% incl). Next: README/CHANGELOG.
- README/CHANGELOG done. Next: full gates + commit.
- committed 5fcd55e locally. Next: fetch/rebase/test/push.
- VERIFY last result: workspace tests green; llvm-cov api 86.12% lines (93.98% excl main); clippy clean; drift test passes; live curl ok (manifest, 302 install, etag).
