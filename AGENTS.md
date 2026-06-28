# Applying these standards

You are working in a repo that vendors this `standards/` submodule. These are the
conventions this fleet of repos follows. Treat them as binding defaults: follow them
unless the repo's own docs explicitly override a specific point.

## How to use this

1. **Identify the archetype.** Read [`docs/overview.md`](docs/overview.md) and decide
   which one this repo is — **full-stack product**, **library**, **orchestration**, or
   **CLI/service tool**. The archetype determines which standards apply and how strictly.
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
- **Testing** — aim for >80% coverage on services; do not unit-test UIs.
- **Branding** — a self-contained, code-as-source-of-truth generator in the repo.
- **Platform UX** — native per OS, designed for the target OS, not ported across.

For services and clients there's a deeper layer of implementation standards — **service
architecture** (`core→db→engine→api`, poem-openapi), **data & persistence** (sqlx, typed
`Id<T>`), **contracts** (OpenAPI/AsyncAPI + drift checks), **error handling**, **auth
integration**, **configuration** (libconfig), **observability** (liblog/OTLP), **security**,
**web/client architecture**, **rust conventions**, and the **`lib*` ecosystem**. All are in
[`docs/`](docs/) and indexed in [`docs/overview.md`](docs/overview.md).

Each bullet is a link or a doc in [`docs/`](docs/). When a standard and the code
disagree, fix the code or flag it — don't silently diverge.
