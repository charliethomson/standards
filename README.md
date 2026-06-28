# standards

Engineering standards for charliethomson's repos — the conventions every product,
library, and orchestration repo is expected to follow.

This repo is **vendored into other repos as a git submodule** at `standards/`. It is
the single source of truth for cross-cutting decisions (versioning, identifiers,
deployment, branding, testing, CI) so individual repos don't each reinvent them, and
so coding agents have one authoritative place to read the rules.

## Layout

- **`AGENTS.md`** — entrypoint. How an agent (or human) applies these standards to a repo.
- **`docs/`** — the standards themselves, one topic per file. `docs/overview.md` is the index.
- **`docs/archetypes/`** — per-repo-type rules. A repo is one of: full-stack product,
  library, orchestration, or CLI/service tool. Start here to know which standards apply.
- **`templates/`** — copyable scaffolding (workflows, `build.rs`, `tarpaulin.toml`,
  `VERSIONING.md`, Komodo stacks, the consuming-repo `AGENTS.md` stub). Copy these into a
  new repo rather than writing them from scratch.
- **`skills/`** — shared Claude Code skills (`<name>/SKILL.md`), usable by any consuming
  repo. Linked into `.claude/skills/` by the setup script.
- **`mcp/`** — shared custom MCP servers, referenced from a consuming repo's `.mcp.json`.

See [`docs/skills-and-mcp.md`](docs/skills-and-mcp.md) for how the shared tooling is wired
into a repo.

## Adding to a repo

```sh
git submodule add https://github.com/charliethomson/standards standards
cp standards/templates/consuming-repo-AGENTS.md AGENTS.md   # then fill in the placeholders
./standards/templates/link-standards.sh                     # link skills, scaffold .mcp.json
```

The root `AGENTS.md` stub points agents into `standards/` so the conventions are
discovered automatically. To update the standards in a consuming repo:

```sh
git submodule update --remote standards
```

## Status

Early/iterative. These docs are designed against the fleet's real products, libraries, and
infrastructure — they describe patterns already in use, not aspirations. Examples are
anonymized to `dev.thmsn.someproduct` / `libsomeproduct`.
