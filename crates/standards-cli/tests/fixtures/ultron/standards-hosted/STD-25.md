# STD-25 journal (worktree .claude/worktrees/agent-a591302f3d8b35caa, branch std-25 off origin/main 5d840ba)

PLAN
- [x] 1. checkout std-25 from origin/main; read PROGRAM/SPECS/task; homelab/mcp check
- [x] 2. vision: start comment with plan
- [x] 3. docs/skills-and-mcp.md rewrite + new docs/migration.md (+ deployment exception & genericized port/IP, ci-cd legacy) -> local commit d87d5b7 (rebased on fb1764d)
- [x] 4. (local commit a3c2b22) README.md, AGENTS.md, overview, glossary (+3 entries), readmes, shortcuts, deployment (public exception), ci-cd (legacy override), archetype repo trees
- [x] 5. (done before 4) local commit cb7c213 templates: consuming-repo-AGENTS.md one-liner, README.* submodule lines, standards/docs comments -> URLs, deprecated headers link-standards.sh + mcp.json
- [x] 6. (local commit b6bcf7e) delete prompts/install.md, prompts/validate-install.md; update prompts/README.md; remove mcp/ + .mcp.json (L19)
- [x] 7. (local commit f1abc96) genericize exposure items (LAN IP, tcp.auth host:port, dev port map, sonarr/fs)
- [x] 8. PUSHED origin/main fb1764d..f1abc96 (d87d5b7 cb7c213 b6bcf7e a3c2b22 f1abc96); cargo test -p standards-core green. verify greps + relative links; commit in logical commits; rebase; push origin HEAD:main
- [x] 9. vision finishing comment (posted). TASK COMPLETE.

STATE
- step 4 (other docs). NOTE: cargo test -p standards-core (CARGO_TARGET_DIR=<worktree>/target) is the link/anchor checker; must pass before push. Keep AGENTS.md "This repo's own code" section (STD-12).

TOUCHED
- d87d5b7 (local): docs/skills-and-mcp.md, docs/migration.md, docs/deployment.md, docs/ci-cd.md

HOMELAB CHECK
- ~/git/homelab/mcp: komodo/server.py identical (diff clean), .env.example identical, README documents grafana via `go install` (homelab .mcp.json uses ~/go/bin/mcp-grafana). standards mcp/grafana only had install.sh (go install wrapper). => safe to delete mcp/ + .mcp.json.

VERIFY
- grep -rn "submodule" docs templates README.md AGENTS.md  (only legacy/migration/vendored upstream hits)
- grep -rn "standards/docs\|standards/templates\|standards/bin" docs templates README.md AGENTS.md
- ls prompts mcp .mcp.json
- relative link check script over changed files
- last result: (not run)
