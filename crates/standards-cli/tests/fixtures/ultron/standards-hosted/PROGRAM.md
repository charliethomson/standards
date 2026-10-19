slug: standards-hosted (vision label `standards-hosted`; projects STD + LIBID)

# Program: consume standards from a hosted server, not a submodule

## SYSTEM CONTEXT

Today every fleet repo (34 under ~/git) vendors charliethomson/standards as a git submodule at
`standards/`. Agents read `standards/docs/*.md` off disk, shared skills reach repos as
`.claude/skills/thmsn-*` symlinks made by `templates/link-standards.sh`, and `bin/standards`
(install/sync/contribute/lint), `bin/janitor` and `bin/ultron` are bash scripts in the submodule.
Submodules drift per repo (dangling skill links in agentutil, auth, registry, homelab), and cargo
pulling libid's submodule broke cold builds.

After this program:
- One Rust workspace in the standards repo (`crates/standards-{core,db,engine,api,cli}`).
  `standards-api` is a public read-only server at https://standards.thmsn.dev that embeds
  docs/, skills/, templates/ and CHANGELOG.md from its own commit. It renders the docs for humans,
  mirrors them as REST for the cli, and serves MCP at /mcp for agents.
- The `standards` cli, installed per machine from the registry, owns setup and drift:
  `doctor` registers the MCP at user scope, writes skill stubs into ~/.claude/skills, adds a
  SessionStart hook that runs `doctor --quiet`, and cleans submodule leftovers in a repo.
  janitor and ultron become cli subcommands.
- Skill stubs carry only name + description locally; the body is fetched with get_skill.
- A repo keeps `.standards.conf` and one `Standards:` line in AGENTS.md.

Invariants an implementer would otherwise guess at:
- standards-core has zero I/O (docs/service-architecture.md:24). Parse from bytes.
- The server serves an allowlist only (docs, skills, templates, CHANGELOG). Never mcp/, bin/,
  prompts/, NOTES.md, dotfiles.
- Content identity = commit sha; binary identity = derived version. Same commit.
- Unmigrated repos must keep working throughout: bin/* and link-standards.sh stay in place.
- The cli never commits; `--fix` stages.
- ultron's state dir `~/.local/state/thmsn/ultron` doesn't move (this file lives there).

## FINDINGS / GOAL

Done = a fresh machine installs the cli, runs `standards doctor --fix`, and then every repo gets
standards, skills and MCP with no submodule; libid has been migrated as the pilot using
docs/migration.md; https://standards.thmsn.dev serves readable docs and a working /mcp.

Out of scope (later program): migrating the other 33 repos, removing the 90
`submodule_override: standards` Woodpecker blocks in 22 repos, deleting bin/* and
link-standards.sh, a real public install path for cloud sessions.

## LOCKED DECISIONS

- L1 Code lives in the standards repo: root Cargo workspace, `crates/standards-{core,db,engine,api,cli}`. Server follows service-architecture: core→db→engine→api, poem + poem-openapi.
- L2 Archetypes: server subtree = full-stack-product with N/A overrides (data-persistence, auth, client-analytics, platform-ux, refresh-tokens, branding, web client) written into this repo's AGENTS.md; cli subtree = cli-tool.
- L3 Identifiers `dev.thmsn.standards.server` / `dev.thmsn.standards.cli`; env prefix `STANDARDS_`; image `ghcr.io/charliethomson/standards-server`; registry product `dev.thmsn.standards`, app `cli`.
- L4 Public at https://standards.thmsn.dev, no auth, read-only, as a documented exception to docs/deployment.md:91 (the content is already public on GitHub). MCP over streamable HTTP at /mcp using rmcp under poem; the spike (STD-13) confirms or swaps to poem-mcpserver.
- L5 Served allowlist: docs/**, skills/** (bodies + assets), templates/**, CHANGELOG.md.
- L6 Routes as in SPECS.md: /, /d/{id}, /t/{path}, /manifest.json, /api/v1/*, /api/docs (Scalar, not /docs), /openapi.json, /health, /version, /api/metrics, /install (302 → https://apps.dev.thmsn.dev/install/dev.thmsn.standards), /mcp. Rendered docs use no inline JS.
- L7 MCP tools: search, get_doc, get_checklist, get_template, get_skill, get_skill_asset, version. One prompt per skill. `instructions` per SPECS.md draft.
- L8 Template rendering happens server-side: strict `{{[A-Z_]+}}`, only given keys replaced, unfilled keys reported. The cli uses the REST mirror, not MCP.
- L9 Manifest carries schema, commit, server_version, min_cli_version (hand-bumped const in core), cli_install_url, archetypes, and per skill both description_sha256 and content_sha256.
- L10 The cli ships through the registry (kind=cli) per docs/registry-publishing.md. Write install, self-update and docs **as if external callers can install from the registry**. User instruction: it's known not to work off-tailnet today, and that's accepted.
- L11 Stub = frontmatter (name, description) + "fetch get_skill and follow it, fail closed". The ownership marker is a `.standards-managed` file in the stub dir. doctor never touches unmarked skill dirs.
- L12 cli surface: doctor, init, new, self-update, janitor, ultron. No `lint` (`doctor --ci` repo scope is the gate), no `contribute`, no `sync`. Exit codes 0/1/2/3. `--quiet` always exits 0 and uses a cached manifest.
- L13 doctor fixes MCP registration via `claude mcp add/remove --scope user` and reads ~/.claude.json read-only. It merges the settings.json SessionStart hook directly (atomic, with a backup, no duplicates).
- L14 `.standards.conf` format unchanged (KEY=VALUE), parsed and never sourced.
- L15 The AGENTS.md line is `Standards: archetype=<a> product=<p> — rules via the \`standards\` MCP (https://standards.thmsn.dev). Repo-specific overrides below.`, matched by the `Standards: ` prefix.
- L16 janitor and ultron are ported behaviour-identically. ultron keeps its state path.
- L17 bin/*, templates/link-standards.sh and templates/mcp.json stay, with deprecated headers, until the fleet sweep.
- L18 Migration in this program = docs/migration.md + the libid pilot only. doctor warns about CI submodule overrides and never rewrites YAML.
- L19 mcp/grafana, mcp/komodo and the repo's .mcp.json leave this repo (homelab has the copies).
- L20 CI is Woodpecker (.woodpecker/, scripts from ~/git/vision/scripts/ci). Versioning is derived; tag v0.1 at go-live; update CHANGELOG.md:4-5.
- L21 Archetype tables are normalized to one linked row per standard. Non-doc rows stay plain text.
- L22 Drop the thmsn-standards-sync and thmsn-standards-contribute skills. Delete prompts/install.md and prompts/validate-install.md (replaced by docs/migration.md).
- L23 The deep-review asset is served through get_skill's asset list + get_skill_asset / REST.
- L24 Coverage ≥80% on the server and cli crates; no unit tests for the rendered HTML; tests are silent and never touch the real HOME.
- L25 Doc links are rewritten to absolute https://standards.thmsn.dev/d/<id>#anchor (templates → /t/<path>) in both the HTML and the MCP/REST markdown.
- L26 The skills rewrite task (STD-24) owns every SKILL.md edit; the janitor and ultron port tasks don't touch skills.
- L27 Archetype marker "**The defining rule**" (orchestration Secrets split row) parses as Defining, same as "**The defining standard**".
- L28 Every file in a skill dir other than SKILL.md is a served asset, at its relative path with nesting kept (thmsn-logs: scripts/obsq.sh, queries/*, overlay-template/*). get_skill returns relative asset paths; REST /api/v1/skills/{name}/assets/{path..} accepts multi-segment paths. (STD-24)
- L29 HAZARD until LIBID-2: origin/main skills now say `standards ultron` and call the MCP. ~/.claude/skills/thmsn-{ultron,jarvis} and ~/.local/bin/ultron symlink into ~/git/standards; do not pull that checkout until the cli + server exist.
- L30 MCP transport (STD-13): rmcp 3.5 (server, macros, transport-streamable-http-server) mounted via poem tower-compat `mcp.compat()` at /mcp; stateless (`with_legacy_session_mode(false)`, `with_json_response(true)`); Host allowlist configurable (STANDARDS_ prefix), default includes standards.thmsn.dev + loopback; prompts hand-written list_prompts/get_prompt per skill; no Caddy flush_interval needed. poem-mcpserver rejected. Paste-ready snippet: std13-spike/FINDINGS-comment.md.
- L31 Coverage gate: tarpaulin breaks on this Mac (rustc 1.99); use `cargo llvm-cov --fail-under-lines 80` locally, STD-18 decides CI tool. Core public API as landed in fb1764d (see STD-12 vision comment); downstream crates build on it, not on the SPECS sketch.
- L32 Docs say `curl -fsSL https://standards.thmsn.dev/install | sh` (STD-25). STD-20 must confirm the registry installer is a shell script or fix docs/migration.md + skills-and-mcp.md. consuming-repo-AGENTS.md keeps the Standards: line plus README/SHORTCUTS bullets (open to user veto).
- L33 Engine API as landed in 5be470e (STD-14 vision comment); commit identity = RELEASE_COMMIT build env, else libbuildinfo; STD-18 must pass RELEASE_COMMIT into the image build.
- L34 api (5fcd55e): bin `standards-server`; mcp::mount and web::mount in lib.rs app(), each task edits only its own module + names its routes in request.rs::api_operation; slash ids sent encoded or raw (paths.rs); ETag=commit, max-age 300; config STANDARDS_BIND, STANDARDS_ALLOWED_HOSTS (AppState.allowed_hosts). /api/metrics is public for now.
- L35 cli (74be252): each subcommand module owns `Args` + `run(ctx, args, io) -> Result<Outcome>`; machine paths come via `Env` (env.rs), tests never touch real HOME; client.rs blocking reqwest; cache.rs ManifestCache (1h, stale fallback, QUIET_TIMEOUT 1s). `curl -fsSL https://standards.thmsn.dev/install | sh` confirmed correct.

## READ FIRST

- `~/.local/state/thmsn/ultron/standards-hosted/SPECS.md`: reconciled contracts (core types, routes, MCP, doctor checks, stub text, bin behaviour with line refs, exposure audit, fleet facts)
- `~/.local/state/thmsn/ultron/standards-hosted/tasks/*.md`: the filed task descriptions
- standards repo: AGENTS.md, docs/service-architecture.md, docs/contracts.md, docs/build-info.md, docs/versioning.md, docs/configuration.md, docs/observability.md, docs/testing.md, docs/ci-cd.md, docs/registry-publishing.md, docs/deployment.md, docs/archetypes/{full-stack-product,cli-tool}.md, docs/skills-and-mcp.md
- bin/standards, bin/janitor, bin/ultron, templates/link-standards.sh, templates/consuming-repo-AGENTS.md
- precedent: ~/git/vision/.woodpecker/, ~/git/vision/scripts/ci/, ~/git/warden/server/api/src/web.rs:37-38 (rust-embed + poem)

## BACKLOG

| Task | Title | Blocked by | Who | State |
|---|---|---|---|---|
| STD-11 | Normalize archetype tables | none | agent | queued |
| STD-13 | Spike rmcp under poem | none | agent | queued |
| STD-24 | Rewrite shared skills | none | agent | queued |
| STD-12 | Workspace + standards-core | STD-11 | agent | queued |
| STD-25 | Rewrite docs/templates + runbook | STD-11 | agent | queued |
| STD-14 | db + engine | STD-12 | agent | queued |
| STD-15 | api skeleton + REST | STD-14 | agent | queued |
| STD-16 | api MCP | STD-15, STD-13 | agent | queued |
| STD-17 | api HTML site | STD-15 | agent | queued |
| STD-20 | cli skeleton init/new/self-update | STD-15 | agent | queued |
| STD-21 | cli doctor | STD-20 | agent | queued |
| STD-22 | port janitor | STD-20 | agent | queued |
| STD-23 | port ultron | STD-20 | agent | queued |
| STD-18 | Docker/compose/Caddy/Woodpecker | STD-15, STD-20 | agent | queued |
| STD-19 | Go live | STD-16, STD-17, STD-18 | human-gated | queued |
| LIBID-2 | Pilot: machine + libid cutover | STD-19, STD-21, STD-24, STD-25 | human-gated | queued |

Parallel from the start: STD-11, STD-13, STD-24. After STD-15: STD-16, STD-17 and STD-20 run in parallel (16/17 both
edit the api crate, so watch for route-registration conflicts). After STD-20: STD-21, STD-22, STD-23.

## DISPATCH LOG

Base for all work: **origin/main** (fetch first). The ~/git/standards main checkout is stale
(behind origin 5, carries a pre-rebase duplicate 6373f59) and has someone else's uncommitted docs
edits; never touch it. Agents work in isolated worktrees and land with
`git fetch origin && git rebase origin/main && git push origin HEAD:main`. SPECS line refs were
taken from a stale tree; treat them as hints and re-locate by content.

| Task | Event | State |
|---|---|---|
| STD-11 | 2026-10-19 wave 1 | dispatched |
| STD-13 | 2026-10-19 wave 1 | dispatched |
| STD-24 | 2026-10-19 wave 1 | dispatched |
| STD-11 | landed 8267e28, verified tables | landed |
| STD-12 | 2026-10-19 wave 2 | dispatched |
| STD-25 | held: user's uncommitted docs edits in ~/git/standards overlap its files | queued |
| STD-24 | landed 3bc9273, verified grep | landed |
| STD-13 | landed (spike, no commit); findings in std13-spike/FINDINGS-comment.md + vision comment | landed |
| STD-25 | 2026-10-19 dispatched, user ok to rebase their docs edits over it | dispatched |
| STD-12 | landed fb1764d; ultron reran core tests 45 pass + clippy clean on origin/main | landed |
| STD-14 | 2026-10-19 dispatched | dispatched |
| STD-25 | landed d87d5b7..f1abc96; ultron reran core tests green on origin/main | landed |
| STD-14 | landed 5be470e; ultron reran workspace tests + clippy green | landed |
| STD-15 | 2026-10-19 dispatched | dispatched |
| STD-15 | landed 5fcd55e; ultron reran tests + live curl (manifest, slash doc id, N/A checklist) | landed |
| STD-16 | 2026-10-19 wave 3 | dispatched |
| STD-17 | 2026-10-19 wave 3 | dispatched |
| STD-20 | 2026-10-19 wave 3 | dispatched |
| STD-17 | ENOSPC mid-build; ultron freed 9G of landed target dirs, resumed from journal step 7 | dispatched |
| STD-18 | 2026-10-19 dispatched early (server parts first; cli pipeline against STD-20 crate once it lands) | dispatched |
| STD-16 | landed 65085a1; ultron reran api tests + live tools/call get_checklist | landed |

## PAUSED 2026-10-19 (user, usage limit)

- Landed: STD-11, 12, 13, 14, 15, 16, 24, 25.
- Mid-flight (told to checkpoint + stop; resume each from its journal, uncommitted work lives in its agent worktree under ~/git/standards/.claude/worktrees/agent-*):
  - STD-17 HTML site: worktree agent-ad8a42086876f2d0a, branch std-17  [stopped: done + verified uncommitted; next = commit, rebase over STD-16 (request.rs, crates/README.md), test, push]
  - STD-20 cli skeleton: worktree agent-ab2dedb6bf4487af0, branch std-20  [stopped: built + tests green, uncommitted; next = README/CHANGELOG, live diff vs bin/standards install, commit, push]
  - STD-18 docker/compose/CI: worktree agent-a16da31646907e505, branch std-18  [stopped: Dockerfile + .dockerignore written, uncommitted; next = local docker build]
- Next queue: STD-21, STD-22, STD-23 in parallel once STD-20 lands.
- Waiting on user: STD-19 go-live (needs STD-18's secrets list), then LIBID-2. Hazard L29: don't pull ~/git/standards main until the cli exists.

## RESUMED 2026-10-19
Journals matched worktrees; nothing landed out of band. STD-17, STD-20, STD-18 resumed in their original agents.

| STD-17 | resumed | dispatched |
| STD-20 | resumed | dispatched |
| STD-18 | resumed | dispatched |
| STD-17 | landed d50f983; ultron reran api tests (35) + reviewed screenshots | landed |
| STD-20 | landed 74be252; ultron reran cli tests (39) + --help | landed |
| STD-21 | wave 4 | dispatched |
| STD-22 | wave 4 | dispatched |
| STD-23 | wave 4 | dispatched |
