# Shared skills & MCP servers

This submodule is also the home for **shared Claude Code skills** and **custom MCP
servers**. Authoring them once here means every repo that vendors `standards/` gets them —
no copy-paste, and an update is a `git submodule update --remote` away.

## Skills

Canonical source: [`../skills/`](../skills/), one directory per skill containing a
`SKILL.md` (plus any scripts/resources the skill needs).

```
standards/skills/
└── <skill-name>/
    ├── SKILL.md
    └── ...            # optional scripts, references
```

Claude Code discovers project skills in `.claude/skills/`, which is *not* where the
submodule lives. So a consuming repo **links** them in:

```sh
# from repo root — done for you by templates/link-standards.sh
mkdir -p .claude/skills
ln -sfn ../../standards/skills/<skill-name> .claude/skills/<skill-name>
```

- Use **relative** symlinks so they resolve for anyone who clones the repo.
- `.gitignore` the symlinks, or commit them — either works; the setup script recreates
  them. Committing them makes the skill visible without running setup.
- Repo-specific skills still live in the repo's own `.claude/skills/`; only shared ones
  are linked from `standards/`.

This standardizes the skills location on `.claude/skills/` (Claude Code's native
convention). Don't use `.agents/skills/`.

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
  `standards` tooling. `.standards.d/` is per-skill and holds behaviour: config, notes,
  and files the skill's scripts load, so a repo can add to a skill without forking it.
- **The skill owns the contract.** Its `SKILL.md` says which files it reads from its
  directory and what each may contain; a skill with nothing to specialise reads nothing.
  A skill with a scaffold ships it (`thmsn-logs`: `obsq.sh init`).
- Commit it. It is the repo's knowledge, reviewed like code, and it outlives the session
  that learned it.
- No secrets: name the env var or env file a value comes from.
- Something every repo would want belongs in the skill, not in each overlay
  (`thmsn-standards-contribute`).

First user: [`thmsn-logs`](../skills/thmsn-logs/SKILL.md) (`SERVICES`, recipes in
`queries/`, extra subcommands in `commands/`, the repo's log notes in `NOTES.md`).

## MCP servers

Canonical source: [`../mcp/`](../mcp/), one directory per server.

```
standards/mcp/
└── <server-name>/
    ├── server.py         # or a built binary / node entry
    └── README.md
```

A consuming repo wires a server into its **root `.mcp.json`** by pointing at the path
inside the submodule. Python servers run via `uv` with inline deps (the homelab pattern);
binaries/node entries run directly.

```jsonc
// .mcp.json  (scaffolded by templates/link-standards.sh — see templates/mcp.json)
{
  "mcpServers": {
    "<server-name>": {
      "command": "uv",
      "args": ["run", "--script", "standards/mcp/<server-name>/server.py"]
    }
  }
}
```

- Reference servers by their `standards/...` path so the submodule stays the source of
  truth; don't copy server code into the repo.
- Keep secrets out of `.mcp.json` — pass them via env (`${VAR}`), consistent with the
  [deployment](deployment.md) secrets rule.

## Adding a new shared skill or server

1. Create it under `standards/skills/<name>/` or `standards/mcp/<name>/`.
2. Document it in that directory's `README.md` / `SKILL.md`.
3. Commit here; consuming repos pick it up on their next `git submodule update --remote`.

The setup helper is [`../templates/link-standards.sh`](../templates/link-standards.sh).
