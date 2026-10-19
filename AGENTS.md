# Applying these standards

These are the conventions this fleet of repos follows, served to every repo by the
`standards` MCP server (https://standards.thmsn.dev). A consuming repo's `AGENTS.md` starts
with a `Standards: archetype=<a> product=<p>` line; treat the standards as binding defaults
and follow them unless that file explicitly overrides a specific point.

**Operational question first?** If you're being asked about the *running system* — is it up
to date in prod, what's live, where are the logs — check the repo's root `SHORTCUTS.md`
before you go spelunking through workflows and compose files. And when you do have to go
spelunking, write the route down there afterwards. See
[shortcuts](https://standards.thmsn.dev/d/shortcuts).

## How to use this

1. **Identify the archetype.** The repo's `Standards:` line (or `.standards.conf`) names
   it: **full-stack product**, **library**, **orchestration**, **CLI/service tool**, or
   **vendored app**. The archetype determines which standards apply and how strictly; see
   [overview](https://standards.thmsn.dev/d/overview).

2. **Get the checklist before changing an area.** `get_checklist(standard, archetype)` on
   the `standards` MCP gives the rule as it applies to this archetype; `get_doc` gives the
   rationale and a copyable implementation; `search` when unsure which standard applies.

3. **Scaffold from templates** (`get_template(path)`, or
   `https://standards.thmsn.dev/t/<path>`) rather than hand-writing config. Copy the
   template, then adjust — don't redesign.

4. **Use the shared tooling.** The `thmsn-*` skills and the MCP registration come from the
   `standards` cli (`standards doctor --fix`). See
   [skills-and-mcp](https://standards.thmsn.dev/d/skills-and-mcp); to move a repo off the
   old `standards/` submodule, see [migration](https://standards.thmsn.dev/d/migration).

## The rules in one breath

- **Versioning** — version is *derived*, never stored: `MAJOR.MINOR.<commit count>`. The
  git tag (`vMAJOR.MINOR`) is the only manual knob. See [versioning](https://standards.thmsn.dev/d/versioning).
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
  root `SHORTCUTS.md`; read before recon, write after. See [shortcuts](https://standards.thmsn.dev/d/shortcuts).
- **READMEs** — every repo (and every independently-buildable subtree) has one, covering the
  mechanism, the layout, the quickstart, and what CI runs. Change the behaviour it describes
  and you update it in the same commit. See [readmes](https://standards.thmsn.dev/d/readmes).

For services and clients there's a deeper layer of implementation standards — **service
architecture** (`core→db→engine→api`, poem-openapi), **data & persistence** (sqlx, typed
`Id<T>`), **contracts** (OpenAPI/AsyncAPI + drift checks), **backups** (`stash-backup` sidecar, NAS, stash audits), **error handling**, **auth
integration**, **configuration** (libconfig), **observability** (liblog/OTLP), **tracing** (always-on +
tail sampling, span shape, the client↔server trace contract), **client analytics** (usage
events relayed through the product server to ClickHouse), **security**,
**web/client architecture**, **rust conventions**, and the **`lib*` ecosystem**. All are served at
https://standards.thmsn.dev and indexed in [overview](https://standards.thmsn.dev/d/overview).

Each bullet is a doc on the server. When a standard and the code
disagree, fix the code or flag it — don't silently diverge. Unsure of a term (a `lib*` crate,
a piece of infra, a convention)? See [glossary](https://standards.thmsn.dev/d/glossary).

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
