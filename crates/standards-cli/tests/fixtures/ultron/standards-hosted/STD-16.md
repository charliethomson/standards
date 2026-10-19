# STD-16 journal: MCP at /mcp

PLAN
- [x] 1. Read PROGRAM/SPECS/spike/api code; branch std-16 off origin/main (5fcd55e)
- [x] 2. Cargo deps (rmcp 3.5, schemars 1, poem tower-compat) in workspace + api Cargo.toml
- [x] 3. mcp.rs: Standards handler, 7 tools (markdown text out, tool errors for misses), prompts per skill, instructions, mount with allowed_hosts; "/mcp" => "mcp" in request.rs
- [x] 4. Tests: tool handlers through the engine (real bundle) + e2e JSON-RPC over HTTP via TestClient
- [x] 5. fmt, clippy -D warnings, cargo test --workspace, llvm-cov -p standards-api >= 80
- [x] 6. Live: run server, rpc.sh/modern.sh walk, claude mcp add --scope local in scratch dir, list, remove
- [x] 7. crates/README.md + CHANGELOG; commit; fetch+rebase; rerun tests/clippy; push origin HEAD:main
- [x] 8. Finishing vision comment with evidence

STATE
- DONE. Finishing comment posted. Landed 65085a1 on origin/main; worktree target/ deleted.
- step 7: README/CHANGELOG then commit. Live check done: walk ok, claude mcp list Connected, registration removed, server stopped; `claude -p` blocked by the worktree guard (not run). NOTE disk was full (174M free); I deleted only my worktree's target/ caches

TOUCHED
- Cargo.toml (workspace rmcp 3.5 + schemars 1.2), crates/standards-api/Cargo.toml (poem tower-compat, rmcp, schemars), src/mcp.rs, src/request.rs ("/mcp" => "mcp"), src/mcp/tests.rs; uncommitted

VERIFY
- cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo llvm-cov -p standards-api --summary-only
- last (pre-rebase): workspace tests all ok (api 34 incl 12 mcp); clippy clean; fmt clean; llvm-cov TOTAL lines 89.25%, mcp.rs 98.31%
