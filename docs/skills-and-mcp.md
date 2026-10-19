# Shared skills & the standards server

The standards, the shared Claude Code skills and the templates reach every repo from one
place: the **standards server** at https://standards.thmsn.dev. A repo carries no copy of
them. It carries a `.standards.conf` and one `Standards:` line in its `AGENTS.md`, and the
machine carries the `standards` cli, which keeps the agent wired to the server.

```
charliethomson/standards (this repo, main)
        │  push → CI builds the server image → Watchtower redeploys
        ▼
https://standards.thmsn.dev ──── /mcp ─────────────▶ Claude Code (user-scope MCP "standards")
        │                   ──── /api/v1, /manifest.json ──▶ standards cli (doctor, init, new)
        └── /d/<id>, /t/<path> (rendered docs, raw templates, for people)
```

## The server

Public, read-only and unauthenticated: the content is already public on GitHub, so this is
a documented exception to the auth rule (see [deployment](deployment.md#public-read-only-exception)).
It embeds `docs/`, `skills/`, `templates/` and `CHANGELOG.md` from the commit it was built
from, and serves nothing else (`bin/`, `prompts/`, `NOTES.md` and dotfiles stay out).

| Route | What it is |
|---|---|
| `/` | Index: archetypes, docs, search |
| `/d/<id>` | A rendered doc. The id is the path under `docs/` without `.md` (`versioning`, `archetypes/cli-tool`) |
| `/t/<path>` | A raw template (`/t/rust/build.rs`) |
| `/manifest.json` | Commit, server version, minimum cli version, archetypes, and each skill's description and hashes |
| `/api/v1/*` | REST mirror of everything above; the cli uses this. Spec at `/openapi.json`, browsable at `/api/docs` |
| `/install` | Redirects to the cli's registry install page |
| `/mcp` | MCP over streamable HTTP |

Relative links between docs in this repo are rewritten to absolute
`https://standards.thmsn.dev/d/<id>` URLs when served, so a doc reads the same on the site,
over REST and over MCP.

### MCP tools

| Tool | Returns |
|---|---|
| `search{query, limit?}` | Ranked doc sections for a query, with anchors |
| `get_doc{id, section?}` | A doc's markdown, or one section of it |
| `get_checklist{standard, archetype}` | The standard's checklist composed with how the archetype applies it (including "not applicable" and why) |
| `get_template{path, vars?}` | A template with the given `{{KEY}}` placeholders filled, plus the keys left unfilled |
| `get_skill{name}` | A skill's body and the paths of its assets |
| `get_skill_asset{name, path}` | One skill asset |
| `version{}` | Server version and content commit |

Every skill is also an MCP prompt of the same name. The server's MCP instructions tell the
agent the order of work: read the archetype from `AGENTS.md` or `.standards.conf`, call
`get_checklist` before changing an area, `get_template` instead of hand-writing config,
`get_doc` for rationale, and `search` when unsure which standard applies.

## The `standards` cli

Installed once per machine from the registry
(`curl -fsSL https://standards.thmsn.dev/install | sh`, see [migration](migration.md#one-time-machine-setup)).
It talks to the server's REST mirror, never to MCP.

| Command | Does |
|---|---|
| `standards doctor [--fix] [--quiet] [--ci] [--only ID..] [--repo PATH]` | Checks the machine and the current repo against the server; `--fix` repairs what it can |
| `standards init` | Writes `.standards.conf`, the `AGENTS.md` line, `SHORTCUTS.md` and a README skeleton into an existing repo. Never overwrites |
| `standards new <archetype> <product>` | Scaffolds a new repo for an archetype into an empty directory |
| `standards self-update [--check]` | Updates the cli from the registry |
| `standards janitor` | Finds and removes stale agent worktrees and target dirs under `~/git` |
| `standards ultron` | Read-only view of in-flight ultron programs |

Exit codes: `0` healthy or warnings only, `1` a check failed, `2` usage error, `3` the server
was needed and unreachable.

### doctor

Two scopes. User scope (skipped with `--ci`) wires the machine; repo scope (skipped outside
a git repo) checks the repo you're in. Each check has an id you can pass to `--only`.

| Id | Checks | `--fix` |
|---|---|---|
| `mcp.registered` | `standards` is a user-scope MCP server pointing at `<host>/mcp` | `claude mcp add -s user -t http standards https://standards.thmsn.dev/mcp` (removing a stale entry first) |
| `mcp.reachable` | `/manifest.json` answers within 2s | warns only |
| `skills.synced` | Every served skill has a current stub in `~/.claude/skills/<name>/`; no orphaned stubs | writes, refreshes or removes stubs |
| `hook.session-start` | `~/.claude/settings.json` runs `standards doctor --quiet` on SessionStart | merges the hook (atomic, backed up, no duplicates) |
| `cli.current` | The cli meets the server's minimum version (fail) and is the latest (warn) | `self-update` |
| `legacy.path-links` | `~/.local/bin/{standards,janitor,ultron}` symlinks into an old `standards/bin` | removes them |
| `repo.conf` | `.standards.conf` parses and names a real archetype | none: run `standards init` |
| `repo.agents` | `AGENTS.md` has the `Standards:` line, matching `.standards.conf` | rewrites or inserts that line |
| `legacy.*` | Leftovers of the old submodule mechanism | see [migration](migration.md#what-doctor---fix-does) |

`--fix` stages changes in the repo and never commits. `--quiet` (what the hook runs) always
exits 0, uses a cached manifest, and prints one line only when something needs attention.
`--ci` runs repo scope only and is the gate a repo's CI can run.

### The SessionStart hook

`doctor --fix` adds this to `~/.claude/settings.json`, so every session starts by noticing
drift (a new skill, a changed description, an outdated cli):

```json
{ "matcher": "startup",
  "hooks": [{ "type": "command", "command": "standards doctor --quiet", "timeout": 10 }] }
```

## Skills

Canonical source: `skills/<name>/SKILL.md` in this repo, plus any assets beside it. The
index is `skills/README.md`.

Claude Code discovers skills from `~/.claude/skills/`. `doctor` writes a **stub** there per
served skill: the frontmatter (`name`, `description`) and an instruction to fetch the body.

```markdown
---
name: thmsn-standards
description: <the description the server serves>
---
This skill's instructions are served by the standards MCP server.
Call `get_skill("thmsn-standards")` on the `standards` MCP server and follow the returned body
exactly, as this skill's content. If the server is unreachable, say so and stop; don't
improvise the procedure.
```

Beside it, a `.standards-managed` file records the hashes the stub was written from. That
marker is how `doctor` knows a directory is its own: it never touches a skill directory
without one, so your other skills (`shadcn`, `vision`, a repo's own) are safe.

What this means for a skill author:

- **The `description` is all a model sees before it fetches.** Keep it trigger-accurate.
- **The body must read correctly with no local files.** Rules come from `get_doc` /
  `get_checklist`, templates from `get_template`, other skills are named by slash command,
  and doc links are absolute.
- **Assets** are listed by `get_skill` and fetched with `get_skill_asset`.

Repo-specific skills still live in the repo's own `.claude/skills/`; only shared ones come
from the server. Don't use `.agents/skills/`.

## Specialising a shared skill: `.standards.d/<skill>/`

A shared skill is identical in every repo, but some need to know things only the repo
does: its service names, the fields its logs carry, the queries worth keeping. That goes
in the consuming repo, never in the skill, under **`.standards.d/<skill-name>/`**:

```
<repo>/.standards.d/
└── <skill-name>/
    ├── config        # KEY=VALUE, read by the skill's scripts after .standards.conf
    ├── NOTES.md      # repo-specific knowledge the skill's SKILL.md tells agents to read
    └── ...           # whatever else the skill defines (recipes, extra commands)
```

- `.standards.conf` stays the repo's identity (product, archetype), read by the
  `standards` cli. `.standards.d/` is per-skill and holds behaviour: config, notes,
  and files the skill's scripts load, so a repo can add to a skill without forking it.
- **The skill owns the contract.** Its `SKILL.md` says which files it reads from its
  directory and what each may contain; a skill with nothing to specialise reads nothing.
  A skill with a scaffold ships it (`thmsn-logs`: `obsq.sh init`).
- Commit it. It is the repo's knowledge, reviewed like code, and it outlives the session
  that learned it.
- No secrets: name the env var or env file a value comes from.
- Something every repo would want belongs in the skill, not in each overlay: contribute it
  here (below).

First user: `thmsn-logs` (`SERVICES`, recipes in `queries/`, extra subcommands in
`commands/`, the repo's log notes in `NOTES.md`).

## Other MCP servers

This repo no longer ships MCP servers. Infra-specific servers (Grafana, Komodo) live with
the infra in the homelab repo and are wired in that repo's `.mcp.json`. A consuming repo
that needs one wires it in its own root `.mcp.json`, with secrets passed via env (`${VAR}`)
as the [deployment](deployment.md) secrets rule requires. Don't reference a server by a
`standards/...` path; there is no `standards/` directory any more.

## Contributing a change

1. Edit this repo (`docs/`, `skills/`, `templates/`) and push to `main`. Genericize first:
   no product names, examples use `someproduct` / `SOMEPRODUCT_` / `libsomeproduct`.
2. CI builds the server image; Watchtower redeploys https://standards.thmsn.dev.
3. Every agent sees the change on its next MCP call. The next `standards doctor` (the
   SessionStart hook) refreshes any stub whose description changed and writes stubs for
   new skills.

The MCP `version` tool (or `/version`) tells you which commit is being served.

## Legacy: the `standards/` submodule

Until the fleet is swept, many repos still vendor this repo as a `standards/` git
submodule, with skills symlinked by `templates/link-standards.sh` and a `.mcp.json` from
`templates/mcp.json`. Those files stay, marked deprecated, so unmigrated repos keep
working. Move a repo off it with the [migration runbook](migration.md).
