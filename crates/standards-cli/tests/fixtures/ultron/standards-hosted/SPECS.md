# standards-hosted — reconciled specs

Condensed from three spec agents (core+server, cli, skills/docs/exposure), 2026-10-19.
Repo: charliethomson/standards (~/git/standards). Paths relative to repo root.

## Content model (core parses, zero I/O)

- docs/*.md (32) + docs/archetypes/*.md (5). No frontmatter. H1 = title. Doc id = stem
  (`versioning`, `archetypes/cli-tool`).
- `## Checklist` is the LAST `##` section of every doc except overview, glossary,
  skills-and-mcp. Items `- [ ] …`, continuation lines indented 6 spaces
  (docs/archetypes/cli-tool.md:66-67).
- Archetype "Standards that apply" table: `| Standard | How it applies here |`
  (cli-tool.md:23). Today N/A rows group short names without links
  (cli-tool.md:36-37, library.md:40, orchestration.md:49, vendored-app.md:58-60) and one
  row isn't a doc ("Secrets split", orchestration.md:48). Bold markers: **N/A**,
  **Default off.**, **Adapted**, **Tooling only.**, **The defining standard**.
  Tables are not exhaustive (readmes/shortcuts missing from several).
- Relative links in docs: `../templates/rust/build.rs`, `archetypes/x.md`,
  `tracing.md#client--server-contract`, `../README.md`. Rewrite to absolute
  `https://standards.thmsn.dev/d/<id>#anchor` (templates → `/t/<path>`).
- Genericized names (`someproduct`, `SOMEPRODUCT_`, `libsomeproduct`) are prose, NOT placeholders.
- skills/<name>/SKILL.md (10): YAML frontmatter with only `name`, `description`.
  One bundled asset: skills/thmsn-deep-review/assets/review.template.md.
  skills/README.md is an index, not a skill.
- templates/ (27 files). Placeholders strictly `\{\{[A-Z_]+\}\}`: PRODUCT×74,
  PRODUCT_UPPER×19, BIN×7, LOOPBACK_PORT×4, SERVICE×3, PRODUCT_KIT×5, ARCHETYPE×2.
  Replace only provided keys; leave everything else (review asset has free-text
  `{{Executive summary — …}}`; templates/branding/build.py:146 has Python `{{ }}`).
  Return `unfilled` keys.
- PRODUCT_UPPER default = PRODUCT upper-cased, `-`→`_` (bin/standards:32).

## Core types (proposal, implementer may refine names, not shape)

```rust
pub struct Bundle { commit, docs: Vec<Doc>, archetypes: Vec<Archetype>, skills: Vec<Skill>, templates: Vec<Template> }
impl Bundle { fn from_files(files: impl IntoIterator<Item=(RelPath, Vec<u8>)>, commit: &str) -> Result<Self> }
pub struct Doc { id: DocId, title, markdown, sections: Vec<Section>, checklist: Option<Checklist> }
pub struct Checklist { doc: DocId, items: Vec<ChecklistItem { text, links }> }
pub enum Applicability { Applies, Defining, Adapted, Optional, NotApplicable }
pub struct ArchetypeRow { standards: Vec<StandardRef>, applicability, note }
pub enum StandardRef { Doc(DocId), Named(String) }
pub struct Skill { name, description, body, assets: Vec<Asset>, content_sha256, description_sha256 }
pub struct Template { path, body, placeholders: BTreeSet<String> }  // render(vars) -> {body, unfilled}
pub struct Manifest { schema: u32, commit, server_version, min_cli_version, cli_install_url,
                      archetypes: Vec<String>, skills: Vec<SkillEntry{name, description, description_sha256, content_sha256, assets}> }
pub struct StandardsConf { product, product_upper, lib: Option, archetype }  // KEY=VALUE parse, never shell
```
engine: `search(q, limit) -> Vec<Hit{doc, anchor, title, snippet, score}>`;
`checklist(standard, archetype) -> ComposedChecklist{applicability, note, standard_items, archetype_items}`
(NotApplicable + reason, not an error).

## Server contract

Served allowlist ONLY: docs/**, skills/** (bodies + assets), templates/**, CHANGELOG.md.
Never mcp/, bin/, prompts/, NOTES.md, dotfiles (.mcp.json has absolute /Users paths).

| Route | |
|---|---|
| GET / | rendered index (archetypes, docs, search box; no inline JS, CSP script-src 'self') |
| GET /d/{id} | rendered doc |
| GET /t/{path} | raw template |
| GET /manifest.json | Manifest |
| GET /api/v1/docs, /docs/{id}, /checklist?standard&archetype, /templates (list), /templates/{path}?KEY=V.., /skills, /skills/{name}, /skills/{name}/assets/{path}, /search?q | REST mirror, poem-openapi; the cli uses this, not MCP |
| GET /api/docs, /openapi.json | Scalar + spec (Scalar NOT at /docs) |
| GET /health, /version, /api/metrics | standard mounts; /version = full libbuildinfo |
| GET /install | 302 → https://apps.dev.thmsn.dev/install/dev.thmsn.standards |
| GET/POST /mcp | MCP streamable HTTP |

MCP tools: `search{query, limit?}`, `get_doc{id, section?}`, `get_checklist{standard, archetype}`,
`get_template{path, vars?}`, `get_skill{name}` (body + asset paths), `get_skill_asset{name, path}`,
`version{}` → {server_version, commit}. One MCP prompt per skill (name = skill name, optional `args`).

MCP `instructions` (draft):
> Fleet engineering standards: binding defaults unless the repo's AGENTS.md overrides them.
> Read the repo's archetype from AGENTS.md or .standards.conf. Before changing an area call
> get_checklist(standard, archetype); use get_template(path) instead of hand-writing config;
> get_doc for rationale; search when unsure which standard applies. A skill whose body says to
> call get_skill: fetch it and follow it as the skill. Check the repo's SHORTCUTS.md before
> operational recon. Prefer this server over any local standards/ copy.

## CLI contract

```
standards [--host URL] [--json] [-v]
  doctor [--fix] [--quiet] [--ci] [--only ID..] [--repo PATH]
  init   [--product] [--archetype] [--lib] [--upper]      # existing repo, no clobber
  new    <archetype> <product> [--lib] [--upper] [DIR]    # empty dir, full scaffold
  self-update [--check]
  janitor [scan|clean] [--root D].. [--days N] [--targets] [--main-targets] [--branches] [-y]
  ultron  [ls|status|watch|log|gc] ...
```
Doctor checks (id: check → --fix):
- user scope (skipped with --ci)
  - `mcp.registered`: ~/.claude.json top-level `mcpServers.standards` == {type:http,url:<host>/mcp} → `claude mcp remove -s user standards; claude mcp add -s user -t http standards <url>`. Read ~/.claude.json read-only (holds oauth keys). No `claude` on PATH → fail, no fix.
  - `mcp.reachable`: GET /manifest.json 200 within 2s → warn only
  - `skills.synced`: every manifest skill has ~/.claude/skills/<name>/ with SKILL.md + `.standards-managed` whose hash matches; no orphan managed dirs; legacy symlinks into */standards/skills replaced → write/replace/remove. Never touch unmarked dirs (shadcn, vision, test-audit, etc).
  - `hook.session-start`: ~/.claude/settings.json has `{"matcher":"startup","hooks":[{"type":"command","command":"standards doctor --quiet","timeout":10}]}` under hooks.SessionStart → merge (no dup, backup, atomic write)
  - `cli.current`: version ≥ min_cli_version (fail) / behind latest (warn) → self-update
  - `legacy.path-links`: ~/.local/bin/{ultron,janitor,standards} symlinks into */standards/bin → remove
- repo scope (skipped outside a git repo)
  - `repo.conf`: .standards.conf parses, archetype valid → none (point at init)
  - `repo.agents`: AGENTS.md has the `Standards: ` line matching conf → rewrite that line / insert
  - `legacy.submodule`: .gitmodules has standards → deinit, git rm, rm .git/modules/standards
  - `legacy.skill-links`: .claude/skills/thmsn-* symlinks into standards/ → git rm/unlink
  - `legacy.mcp-json`: .mcp.json is the untouched template (`//` key + empty servers) or has `standards/` paths → delete file/entry
  - `legacy.ci-override`: `submodule_override: standards` in .woodpecker/*, `submodules: true` in workflows → warn with file:line, never rewrite YAML
Exit: 0 ok/warn, 1 fail (in --ci: repo-scope only), 2 usage, 3 server unreachable when needed.
--quiet: always 0, prints one line only when unhealthy, uses cached manifest
(~/.cache/thmsn/standards/manifest.json, refresh ≤ hourly, 1s timeout). --fix stages, never commits.

Stub SKILL.md:
```
---
name: <name>
description: <server description>
---
This skill's instructions are served by the standards MCP server.
Call `get_skill("<name>")` on the `standards` MCP server and follow the returned body
exactly, as this skill's content. If the server is unreachable, say so and stop; don't
improvise the procedure.
```
`.standards-managed` beside it: `content_sha256=<…>\ndescription_sha256=<…>\n`.

AGENTS.md line: `Standards: archetype=<a> product=<p> — rules via the \`standards\` MCP (https://standards.thmsn.dev). Repo-specific overrides below.`

## Existing bin/ behaviour to port exactly

- bin/janitor (233 lines): scan default / clean; flags :43-56; repo discovery find -maxdepth 3 :86-89;
  in-use lsof -a -d cwd :62-66; cargo target needs CACHEDIR.TAG :70-74; age = newest of index mtime,
  logs/HEAD mtime, last commit :120; --no-optional-locks :127; decision order :132-140;
  --force only with .gitmodules + clean recheck :210-218; worktree prune :231; non-interactive
  needs --yes :201.
- bin/ultron (357 lines): state ${XDG_STATE_HOME:-~/.local/state}/thmsn/ultron/<slug>/; commands
  :349-357; backlog parse awk :93-141 (first cell = task id, last row wins, nearest state word of
  landed/done/dispatched/in-flight/blocked/queued, gated = human-gated|[HUMAN]|🔒); plan progress
  [x]/[ ] :144-150; state line :153-166; watch alt-screen, redraw on fingerprint change, plain once
  when not a TTY, waits for a program :259-311; gc always prompts :336. ~21 live programs depend on it.
- bin/standards install :100-142 → `init`: writes .standards.conf, AGENTS.md (from template),
  SHORTCUTS.md :76-83, README (library/cli-tool → README.library.md else README.product.md :85-98),
  never overwriting.

## Exposure audit (repo is already public on GitHub, so nothing new is exposed)

Genericize while rewriting docs (low urgency): LAN IP 192.168.0.193 (glossary.md:73,
deployment.md:88-89, templates/deploy/compose/prod.compose.yml:24); tcp.auth.dev.thmsn.dev:7070
(templates/rust/config.rs:14, configuration.md:85, templates/deploy/komodo/sync/stacks.toml:39-41);
dev box port map (deployment.md:32, prod.compose.yml:3); sonarr/fs examples in identifiers.md:83-87.

## Consuming fleet (for the later sweep)

34 repos under ~/git have the submodule; all but libci have .standards.conf. 90 Woodpecker files in
22 repos carry `submodule_override: standards`; kiln/.github/workflows/ci.yml:82,149,186 has
`submodules: true`. No pipeline executes standards/bin/*. Pilot = libid (cargo git dep of ~10 repos;
cargo pulling its submodule broke cold builds, see kiln .gitmodules comment THM-330/361);
fallback igrip.
