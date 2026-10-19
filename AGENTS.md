# Applying these standards

You are working in a repo that vendors this `standards/` submodule. These are the
conventions this fleet of repos follows. Treat them as binding defaults: follow them
unless the repo's own docs explicitly override a specific point.

**Operational question first?** If you're being asked about the *running system* — is it up
to date in prod, what's live, where are the logs — check the repo's root `SHORTCUTS.md`
before you go spelunking through workflows and compose files. And when you do have to go
spelunking, write the route down there afterwards. See [`docs/shortcuts.md`](docs/shortcuts.md).

## How to use this

1. **Identify the archetype.** Read [`docs/overview.md`](docs/overview.md) and decide
   which one this repo is — **full-stack product**, **library**, **orchestration**,
   **CLI/service tool**, or **vendored app**. The archetype determines which standards apply
   and how strictly.
   The per-archetype rules live in [`docs/archetypes/`](docs/archetypes/).

2. **Read the relevant topic docs** in [`docs/`](docs/) before making changes in that
   area. Each doc states the rule, the rationale, and a copyable implementation.

3. **Scaffold from [`templates/`](templates/)** rather than hand-writing config. Copy the
   template, then adjust — don't redesign.

4. **Use the shared tooling.** Skills in [`skills/`](skills/) and MCP servers in
   [`mcp/`](mcp/) are available to every repo that vendors this submodule. See
   [`docs/skills-and-mcp.md`](docs/skills-and-mcp.md) for how they're linked in.

## The rules in one breath

- **Versioning** — version is *derived*, never stored: `MAJOR.MINOR.<commit count>`. The
  git tag (`vMAJOR.MINOR`) is the only manual knob. See [`docs/versioning.md`](docs/versioning.md).
- **Identifiers** — reverse-domain `dev.thmsn.<product>[.<component>]`, everywhere
  (`<component>` = a service like `server`/`worker`/`cli` or an app surface like
  `ios`/`macos`/`winui`/`web`; grants are a parallel axis).
- **Build info** — embed detailed build metadata via `libbuildinfo` when feasible.
- **Workflow** — single developer, single user. Commit straight to `main`. No PR gates.
- **Deployment** — Komodo GitOps + Watchtower + Caddy; client apps publish to the
  `apps.dev.thmsn.dev` registry.
- **CI** — self-hosted runners (managed by agentutil) for Linux/macOS builds.
- **Testing** — aim for >80% coverage on services; do not unit-test UIs. A test that only
  mirrors the implementation (mock everything, verify the calls) is deleted, not maintained.
  **Tests never make sound**: every audio sink no-ops under test, Playwright runs `--mute-audio`.
- **Branding** — a self-contained, code-as-source-of-truth generator in the repo.
- **Platform UX** — native per OS, designed for the target OS, not ported across.
- **Shortcuts** — repeated operational questions get one-command answers in the repo's
  root `SHORTCUTS.md`; read before recon, write after. See [`docs/shortcuts.md`](docs/shortcuts.md).
- **READMEs** — every repo (and every independently-buildable subtree) has one, covering the
  mechanism, the layout, the quickstart, and what CI runs. Change the behaviour it describes
  and you update it in the same commit. See [`docs/readmes.md`](docs/readmes.md).

For services and clients there's a deeper layer of implementation standards — **service
architecture** (`core→db→engine→api`, poem-openapi), **data & persistence** (sqlx, typed
`Id<T>`), **contracts** (OpenAPI/AsyncAPI + drift checks), **backups** (`stash-backup` sidecar, NAS, stash audits), **error handling**, **auth
integration**, **configuration** (libconfig), **observability** (liblog/OTLP), **tracing** (always-on +
tail sampling, span shape, the client↔server trace contract), **client analytics** (usage
events relayed through the product server to ClickHouse), **security**,
**web/client architecture**, **rust conventions**, and the **`lib*` ecosystem**. All are in
[`docs/`](docs/) and indexed in [`docs/overview.md`](docs/overview.md).

Each bullet is a link or a doc in [`docs/`](docs/). When a standard and the code
disagree, fix the code or flag it — don't silently diverge. Unsure of a term (a `lib*` crate,
a piece of infra, a convention)? See [`docs/glossary.md`](docs/glossary.md).

## The shell is zsh

The Bash tool runs **zsh** on the dev machines, not bash. zsh does not word-split unquoted
parameter expansions, so this runs `"cargo test --workspace"` as one command name and fails:

```sh
cmd="cargo test --workspace"
$cmd            # zsh: command not found: cargo test --workspace
```

Don't stash a command line in a string variable. Write the command out, or use an array
(`cmd=(cargo test --workspace); "${cmd[@]}"`), or a function. Same for lists of args or
files: `files="a b c"; rm $files` is one argument in zsh, so use `files=(a b c)` instead.
`${=var}` forces splitting if you really need it.

---

## This repo's own code

Everything above is for repos that consume these standards. This section is for changes to
the Rust workspace in this repo ([`crates/`](crates/README.md)): the server behind
https://standards.thmsn.dev and the `standards` cli. It follows these standards like any
other fleet repo, with these choices recorded:

- **Archetypes.** The server crates (`standards-core`, `-db`, `-engine`, `-api`) are a
  [full-stack product](docs/archetypes/full-stack-product.md) with these rows **N/A**:
  data & persistence (the content is embedded at build time; there is no database),
  auth integration and refresh tokens (public, read-only, no accounts), client analytics,
  platform UX and the web client (the site is server-rendered docs with no client app), and
  branding. The cli (`standards-cli`) is a [CLI tool](docs/archetypes/cli-tool.md).
- **Identifiers.** `dev.thmsn.standards.server` and `dev.thmsn.standards.cli`; env prefix
  `STANDARDS_`.
- **Layers.** `core → db → engine → api` ([service architecture](docs/service-architecture.md));
  the cli depends on `core`, never on the server crates. `standards-core` does no I/O.
- **Served content.** Only `docs/**`, `skills/**`, `templates/**` and `CHANGELOG.md`, decided
  by `standards_core::is_served`. Never `bin/`, `mcp/`, `prompts/`, `NOTES.md` or dotfiles.
- **Versioning.** Derived ([versioning](docs/versioning.md)); content identity is the commit
  the server embeds. `MIN_CLI_VERSION` in `standards-core` is bumped by hand when the
  manifest or REST shape changes in a way an older cli would misread.
- **Tests** read this repo's real content from disk and hand it to core as bytes; when a doc
  edit breaks one (a lost checklist, a broken link), fix the doc.
