# STASH-6 — M3: the search index
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-a5bc62b21e0fec6a7 (branch worktree-agent-a5bc62b21e0fec6a7)

PLAN
- [x] 1. Read AGENTS/DESIGN/README/standards + source; vision in_progress + start comment
- [x] 2. Migration 0004 (fix FTS over WITHOUT ROWID if 0003 didn't; seen_at, dir mtime for refresh)
- [x] 3. stash-storage: walk (crawl) + readdir-one-dir, skip .stash, links not followed, sentinel recheck
- [x] 4. stash-db: search module (search query, dir mtime read, upsert batch, sweep)
- [x] 5. stash-engine: crawler (periodic, paced, abort on sentinel loss) + refresh (single-flight, debounce)
- [x] 6. api: config (crawl interval etc), metrics, spawn crawler in main, refresh hook in listEntries, GET /search op + route_name + opId set + read allowlist; tests
- [x] 7. Regenerate api/openapi.json; web: regen client, search UI; web CI green
- [x] 8. Docs: README, AGENTS Status, DESIGN milestones
- [x] 9. fmt/clippy/test/tarpaulin >= 80%; commit; vision finish comment

STATE
- DONE. Commit 331dfb5 on worktree-agent-a5bc62b21e0fec6a7. Vision finish comment posted; issue left in_progress for the orchestrator.

TOUCHED
- server/db/migrations/0004_crawl.sql, server/storage/src/walk.rs, server/storage/src/lib.rs (pub mod walk), server/db/src/index.rs (put_many/sweep/sweep_children/record_crawl/CrawlRun), server/db/src/search.rs (new), server/db/src/lib.rs, server/engine/src/{crawl,refresh,lib}.rs, engine/Cargo.toml (sqlx dev-dep), api/src/{state,routes,app,config,main,metrics}.rs, api/src/routes/search.rs (new), api/Cargo.toml (sqlx dev-dep), api/openapi.json, apps/web/src/{App.tsx,api/client.ts,api/keys.ts,api/schema.d.ts,components/Search.tsx}, README.md, AGENTS.md, DESIGN.md, server/README.md, api/README.md, deploy/compose/prod.compose.yml (comment)
- vision: in_progress + start comment posted

VERIFY
- cd server && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
- cargo tarpaulin --engine llvm (in server/)
- web CI per .woodpecker/web.ci.yml
- LAST RESULT: fmt clean, clippy clean, 354 tests pass, tarpaulin 92.94%, openapi regen idempotent after commit, web codegen clean + typecheck + build green
