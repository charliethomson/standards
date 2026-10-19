# STASH-11 — GET /api/metrics does not exist, and Prometheus is already scraping it

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a29c346f537cbe941
Branch: main (commit straight to main, no PR). Another agent pushes to main concurrently.

## PLAN
- [x] 1. Deps: drop `metrics` + `metrics-exporter-prometheus` from server/Cargo.toml,
      api/Cargo.toml and engine/Cargo.toml (all three were declaration-only — nothing in
      the workspace imported either crate). Add `prometheus-client = "0.25"` (matches
      vision). Keep `subtle`.
- [x] 2. New `server/api/src/metrics.rs`: one Registry, `stash_*` families, `render()`
      that recomputes the per-root gauge from the sentinel before encoding.
- [x] 3. `state.rs`: AppState carries `Metrics` + `metrics_token`, plus a `roots()`
      accessor for the exposition (grants deliberately not consulted — a scraper is not
      a user whose grants decide which mounts may be reported as broken).
- [x] 4. `main.rs`: mount `/api/metrics` as a plain poem `#[handler]`, NOT poem-openapi.
      `subtle::ConstantTimeEq` against metrics_token; 404 when the token is unset.
      OPENMETRICS content-type const lives next to the family definitions.
- [x] 5. `main.rs`: `observe()` middleware recording requests / duration / bytes.
- [x] 6. Tests: gated 401/404/200, content type, `# EOF`, root_degraded flips at render,
      route label never a path, contract-vs-route-table drift gate.
- [x] 6b. Moved the app assembly out of main.rs into a new `server/api/src/app.rs`.
      `server/tarpaulin.toml` excludes main.rs as "bootstrap wiring", which stopped being
      true the moment a constant-time token compare lived there. app.rs is covered
      (65/74 lines).
- [x] 7. Gates (commands from .woodpecker/rust.ci.yml, run from server/). All green;
      coverage 87.03% (was 85.87%).
- [x] 8. Docs: AGENTS.md status + 4 new repo-specific overrides; README.md known-gaps
      bullet removed and the observability section rewritten with the series table.
      DESIGN.md needed nothing — it already described the route as existing.
      Also corrected README's stale "Komodo stack on the **fs** box" to **dev**
      (ccf8bec moved it; the README line was missed).
- [x] 9. /thmsn-standards-review on the diff. Three findings, all fixed:
      (a) the new request middleware logged a line but opened no span —
          observability.md's checklist wants requests wrapped in one. Added
          `info_span!("http.request", method, route, status)` via `Instrument`
          (never an `enter()` guard across an await).
      (b) state.rs doc-referenced "the handler in main.rs" after the app.rs split.
      (c) `cargo tarpaulin --out Xml` drops `server/cobertura.xml`, untracked and
          not gitignored — one `git add -A` away from committing a coverage report.
          Added to .gitignore.
- [x] 10. Rebase. First push was rejected — the concurrent agent had landed 9f62c22
       (auth over TLS) and ea7ef67 (ignore agent worktrees). Rebased onto those; the
       only conflict was .gitignore, where both additions belong (kept both).
       `main.rs` auto-merged cleanly: their TLS args on `Auth::connect` sit above my
       metrics block. Contract regenerated after the rebase — still unchanged, so no
       web codegen regen was needed either. All six gates re-run on the rebased tree.
- [x] 11. Pushed: ea7ef67..7fe0216 -> main.

## STATE
**DONE.** Commit 7fe0216 is on origin/main. All six gates green on the pushed tree.
Nothing half-applied, nothing outstanding.

NOTE: an earlier attempt to write this journal via a bash heredoc reported success but
never landed — bash writes in this session are sandboxed. Use the Write tool for this
file, not shell redirection.

Three behaviours found the hard way, worth not re-discovering:
1. poem's `map_to_response()` maps only the Ok side. An `.around` doing
   `ep.call(req).await?` silently drops EVERY error response, so 401/404/503 went
   uncounted. `observe` now resolves the Err into `error.into_response()` itself.
2. `poem-openapi` stamps `OperationId` only on a response an operation actually
   produced, so vision's operationId-off-the-response trick attributes no 401 and no
   grant-404 to its route. `route_name` derives the name from path segments instead
   (root segment folded out); `every_contract_operation_has_a_route_name` walks the
   emitted spec so the hand-written table cannot drift from the contract.
3. `promised_bytes` reads Content-Length, which only the download route sets explicitly
   (download.rs); poem sizes JSON payloads below where an endpoint can see them. So the
   bytes counter is file bytes — documented as such rather than pretended otherwise.

Also extracted `build_app(state, web_dir, metrics)` out of `run()` so the mount map is
testable without a listener, a real auth connection or a mounted NAS.

## TOUCHED
- server/Cargo.toml         metrics + metrics-exporter-prometheus -> prometheus-client 0.25
- server/api/Cargo.toml     same swap
- server/engine/Cargo.toml  dropped metrics (declaration-only there too)
- server/Cargo.lock         regenerated
- server/api/src/metrics.rs NEW — registry, stash_* families, render()
- server/api/src/state.rs   AppState carries Metrics + metrics_token; roots() accessor
- server/api/src/app.rs     NEW — mount map (build), metrics_endpoint, route_name,
                            promised_bytes, observe, api_service/openapi_spec + tests
- server/api/src/main.rs    now bootstrap only; mod app; mod metrics; calls app::build
                            and app::openapi_spec
- server/api/src/routes.rs  test call site of AppState::new
- AGENTS.md                 status line; 4 new repo-specific overrides
- README.md                 known-gaps bullet dropped; observability section rewritten;
                            fs box -> dev box; 94 tests / 87.03%
Commits: none yet.

## VERIFY
Run from `<worktree>/server`:
  cargo fmt --all -- --check
  cargo build --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo run -q --bin stash-api -- openapi > ../api/openapi.json
  git diff --exit-code -- ../api/openapi.json
  cargo tarpaulin --engine llvm --out Xml     # floor 80%, was 85.87%
Last result (all six run on the pushed tree, 7fe0216, 2026-09-21):
  fmt      clean
  build    clean
  clippy   clean (-D warnings)
  test     96 passed / 0 failed  (56 stash-api + 39 storage + 1 core)
  drift    api/openapi.json unchanged — the metrics route is correctly outside the contract
  coverage 86.10%, 514/597 lines (floor 80; was 85.87 before this work)

## WHAT IS MEASURED
- stash_http_requests_total{route,status_class}
- stash_http_request_duration_seconds{route}   (1ms..~33s, 16 exponential buckets)
- stash_http_response_bytes_total{route}       (Content-Length; in practice file bytes)
- stash_root_degraded{root}                    recomputed from the sentinel at render
- stash_build_info{version}, stash_metrics_scrapes_total

Deliberately NOT measured yet: anything about writes, the index/crawl or thumbnails
(M2–M4, all stubs), the auth me-cache hit rate, and a per-root bytes/requests split
(route x root would multiply series for a number `stash_root_degraded` plus the route
counters already answer).

## NOTES
- standards/docs/observability.md is binding: prometheus-client,
  `application/openmetrics-text; version=1.0.0; charset=utf-8`, `<product>_*` names.
- Reference: ~/git/vision/server/engine/src/metrics.rs and
  ~/git/vision/server/api/src/openapi.rs (metrics_endpoint).
- stash has no openapi.rs; the mount map is in main.rs (now in `build_app`).
- /api/metrics must NOT enter the poem-openapi surface; api/openapi.json unchanged.
- The standards submodule was uninitialised in this worktree; `git submodule update
  --init standards` fixed it (no repo change).
