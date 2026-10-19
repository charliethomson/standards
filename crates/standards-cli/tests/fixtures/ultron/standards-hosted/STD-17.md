# STD-17 journal: rendered docs site (/ and /d/{id})

PLAN        — [x] done  [ ] not yet
- [x] 1 setup: worktree agent-ad8a42086876f2d0a reset to origin/main (5fcd55e) on branch std-17; read PROGRAM/SPECS/code
- [x] 2 vision start comment with plan
- [x] 3 web module WRITTEN, NOT YET COMPILED: web.rs (Site prerendered at startup, handlers, mount, CSP/security headers via SetHeader, caching::Immutable), web/render.rs (pulldown-cmark: links -> root-relative, heading ids from core anchors + self-link, tables in .scroll, task items -> li.task/ul.tasks with span.box), web/page.rs (Shell layout, index, doc, search, not_found), web/static/{site.css,search.js}
- [x] 4 request.rs: api_operation names web_index/web_search/web_doc/web_static; is_html() so text/html error bodies are not JSON-rewritten
- [x] 5 api Cargo.toml: pulldown-cmark html feature
- [x] 6 smoke test written: web/tests.rs (every doc 200, no absolute links left, every /d/ href resolves, 404 html, static + search 200)
- [x] 7 BUILD: cargo build -p standards-api; fix compile/clippy errors
- [x] 8 run server on a free port, visual check (desktop/375, light/dark, /d/ci-cd, /d/archetypes/cli-tool, checklist, anchor, search JS + no-JS); screenshots to std17-shots/
- [x] 9 CHANGELOG + crates/README.md
- [x] 10 cargo test --workspace, clippy -D warnings, commit, rebase origin/main, push HEAD:main
- [x] 11 vision finish comment

STATE       — DONE: d50f983 on origin/main, target/ deleted, finishing vision comment posted.
              All source edits are complete on disk and uncommitted in the worktree; nothing half-applied.
              Landed tasks' worktrees still hold target dirs: std-15 4.1G, std-12 1.2G, std-14 1.1G, std-25 320M (not mine to delete).
TOUCHED     — crates/standards-api/src/web.rs, web/{render,page,tests}.rs, web/static/{site.css,search.js}, request.rs (api_operation arms + is_html), api Cargo.toml (pulldown html), Cargo.lock, CHANGELOG.md, crates/README.md; commit d50f983 (on main)
              crates/standards-api/src/request.rs (api_operation arms + is_html), crates/standards-api/Cargo.toml; no commits
VERIFY      — cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; live curl every doc id. Not yet run (ENOSPC).
