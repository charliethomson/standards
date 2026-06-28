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
- **`bin/standards`** — the integration CLI (`install` / `sync` / `contribute` / `lint`).
- **`prompts/`** — copy-paste prompts to hand an agent (e.g. [the install prompt](prompts/install.md)).

See [`docs/skills-and-mcp.md`](docs/skills-and-mcp.md) for how the shared tooling is wired
into a repo.

## Adding to a repo

```sh
git submodule add git@github.com:charliethomson/standards standards
standards/bin/standards install --product <slug> [--upper ENV] [--lib name] [--archetype <type>]
```

`install` writes a root `AGENTS.md` (pointing agents into `standards/`), records the product's
identity in `.standards.conf`, links shared skills into `.claude/skills/`, and scaffolds `.mcp.json`.

## Keeping in sync

```sh
standards/bin/standards sync
```

Pulls upstream, prints the changelog of what changed, re-links any new skills, and stages the
submodule pointer bump for you to commit.

## Contributing a change upstream

Edit files under `standards/`, then:

```sh
standards/bin/standards contribute -m "describe the change"
```

It **genericizes this repo's identifiers** (`<slug>` → `someproduct`, your env prefix →
`SOMEPRODUCT_`, your lib → `libsomeproduct`), verifies none of your product's names remain,
pushes to the standards repo, and stages the pointer bump here. `standards lint` runs that
verification on its own — it reads your `.standards.conf` (which lives in *your* repo), so **no
product names are ever stored in this repo**. See [`bin/standards`](bin/standards).

## Status

Early/iterative. These docs are designed against the fleet's real products, libraries, and
infrastructure — they describe patterns already in use, not aspirations. Examples are
anonymized to `dev.thmsn.someproduct` / `libsomeproduct`.
