# STD-13 — Spike: rmcp streamable HTTP under poem

PLAN
- [x] 1. Read context (PROGRAM L4/L7, SPECS server contract, service-architecture, rust-conventions, Caddyfile, warden web.rs)
- [x] 2. Start comment on vision STD-13 with plan (comment K27HGE6PTS7)
- [x] 3. Scaffold spike crate in scratch dir: poem 3.1 + rmcp (transport-streamable-http-server) via TowerCompatExt at /mcp + /health
- [x] 4. One tool (version), one prompt (optional args), instructions; build
- [x] 5. Verify via curl JSON-RPC (initialize, tools/list, tools/call, prompts/list, prompts/get); stateful vs stateless
- [x] 6. Verify via claude mcp add --scope local in scratch dir; then remove (claude -p could not auth: OAuth expired; used stream-json init event + debug log instead)
- [x] 7. Caddy check (local caddy v2.11.4) with template Caddyfile shape; SSE flush -> flush_interval -1 NOT needed
- [x] 8. poem-mcpserver 0.3.1: source-inspected only (tower-compat worked). No prompts/get, prompts/list hardcoded empty, static instructions, no server/discover -> rejected
- [x] 9. Finishing comment on STD-13 posted; report

STATE
DONE. Finishing comment posted on STD-13 (body: std13-final.md in scratchpad; copy below in vision). All servers/caddy stopped. Local MCP removed.

TOUCHED
- spike (scratch, may be reaped): /private/tmp/claude-501/-Users-charlesthomson-git-standards--claude-worktrees-standards-hosted-program-945710/1f506895-6acc-4bbd-b5a2-a9210acb0e0d/scratchpad/std13-spike
- DURABLE copy of spike source + scripts + transcripts (no target/): ~/.local/state/thmsn/ultron/standards-hosted/std13-spike/
- nothing in the standards repo; no commits; no branch pushed

VERIFY
- cargo build (std13-spike): clean, 0 warnings
- ./rpc.sh http://127.0.0.1:8099/mcp 2025-06-18  -> initialize has instructions, tools/list, tools/call, prompts/list, prompts/get all 200 (out-stateful.txt; stateless out-stateless.txt)
- ./modern.sh http://127.0.0.1:8098/mcp -> server/discover w/ instructions, tools/call, prompts/list 200 (out-modern-final.txt)
- claude stream-json init: std-spike connected, tool mcp__std-spike__version, slash mcp__std-spike__thmsn-copy (out-claude-init.jsonl); debug log negotiatedProtocolVersion 2026-07-28
- ./caddy-test.sh -> priming event +0.04s, keepalive +15s identical for plain / flush -1 / encode (out-caddy-*.txt)
