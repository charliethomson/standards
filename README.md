# standards

Engineering standards for charliethomson's repos — the conventions every product,
library, and orchestration repo is expected to follow.

This repo is the single source of truth for cross-cutting decisions (versioning,
identifiers, deployment, branding, testing, CI) so individual repos don't each reinvent
them, and so coding agents have one authoritative place to read the rules. It is **served**
at https://standards.thmsn.dev: rendered docs for people, a REST mirror for the `standards`
cli, and an MCP endpoint for agents. Consuming repos carry no copy of it.

## Layout

- **`AGENTS.md`**: how an agent applies these standards, and (below its divider) the rules
  for working on this repo's own code.
- **`docs/`**: the standards themselves, one topic per file. `docs/overview.md` is the index.
- **`docs/archetypes/`**: per-repo-type rules. A repo is one of: full-stack product,
  library, orchestration, CLI/service tool, or vendored app. Start here to know which
  standards apply.
- **`templates/`**: copyable scaffolding (workflows, `build.rs`, `tarpaulin.toml`,
  `VERSIONING.md`, Komodo stacks, the consuming-repo `AGENTS.md`, `SHORTCUTS.md` and
  `README` stubs), served raw at `https://standards.thmsn.dev/t/<path>`.
- **`skills/`**: shared Claude Code skills (`<name>/SKILL.md` plus assets). Machines get a
  stub per skill from `standards doctor`; the body is fetched from the server.
- **`crates/`**: the Rust workspace behind https://standards.thmsn.dev and the `standards`
  cli; see [`crates/README.md`](crates/README.md).
- **`bin/`**: **deprecated** bash tools (`standards`, `janitor`, `ultron`) kept for repos
  still on the old submodule until the fleet is migrated. The cli replaces them.
- **`prompts/`**: copy-paste prompts for one-offs (see [`prompts/README.md`](prompts/README.md)).
  Not served.

The server serves `docs/`, `skills/`, `templates/` and `CHANGELOG.md` only. See
[skills-and-mcp](https://standards.thmsn.dev/d/skills-and-mcp) for the whole mechanism.
For a chat client that imports one URL rather than crawling, the whole corpus is at
[`/llms-full.txt`](https://standards.thmsn.dev/llms-full.txt) and its index at
[`/llms.txt`](https://standards.thmsn.dev/llms.txt).

## Using it

Once per machine:

```sh
curl -fsSL https://standards.thmsn.dev/install | sh   # installs the `standards` cli
standards doctor --fix    # registers the MCP, writes skill stubs, adds the SessionStart hook
```

In a repo:

```sh
standards init --product <slug> --archetype <type>   # existing repo; never overwrites
standards new <archetype> <slug>                     # new, empty directory
standards doctor                                     # check the repo; --fix repairs, --ci gates
```

`init` writes `.standards.conf`, the `Standards:` line at the top of `AGENTS.md`, an empty
`SHORTCUTS.md` and a README skeleton. A repo still vendoring the old `standards/` submodule
moves across with the [migration runbook](https://standards.thmsn.dev/d/migration).

## Contributing a change

Edit `docs/`, `skills/` or `templates/` here and push to `main`. CI builds the server image
and Watchtower redeploys it; agents see the change on their next MCP call, and the next
`standards doctor` refreshes skill stubs. Genericize before you push: no product names,
examples use `someproduct` / `SOMEPRODUCT_` / `libsomeproduct`.

## CI and deploy

Woodpecker (`.woodpecker/`): `rust.ci` runs fmt, clippy, tests, the OpenAPI drift check and
the 80% coverage gate over the workspace, and runs on doc edits too, since the content is
compiled in; `server.build` pushes the server image to GHCR; `cli.release` publishes the cli
to the registry; `audit.ci` audits `Cargo.lock` weekly. Details and the local commands are in
[`crates/README.md`](crates/README.md#ci-and-delivery); the Komodo stack is in
[`deploy/`](deploy/README.md).

## Status

Early/iterative. These docs are designed against the fleet's real products, libraries, and
infrastructure — they describe patterns already in use, not aspirations. Examples are
anonymized to `dev.thmsn.someproduct` / `libsomeproduct`.
