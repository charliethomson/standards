# Overview & archetypes

These standards describe how this fleet of repos is built. They are descriptive of
patterns already in use across the reference repos, not aspirational.

## Operating context (read this first)

Every standard here assumes the same context. It is deliberately narrow:

- **Single developer, single user.** Optimise for one person moving fast, not for teams.
  Commit directly to `main`; no PR review gates, no release trains. See
  [`workflow.md`](workflow.md).
- **Self-hosted everything.** CI runs on self-hosted agents (managed by agentutil),
  deploys land on the homelab via Komodo, client apps ship through the self-hosted
  `apps.dev.thmsn.dev` registry. There is no reliance on external SaaS for the core loop.
- **Reverse-domain identity.** Everything namespaced under `dev.thmsn.*`.
- **Versions are cheap and disposable.** Because there's one user and continuous deploy to
  `main`, versions exist for *debugging* ("which commit is this?"), not for release
  marketing. Hence the derived `MAJOR.MINOR.<commit count>` scheme.

## Pick the archetype

A repo is exactly one of these. The archetype tells you which standards are mandatory vs
optional. Full rules in [`archetypes/`](archetypes/).

| Archetype | What it is | Reference repos | Standards that apply hardest |
|---|---|---|---|
| **[Full-stack product](archetypes/full-stack-product.md)** | Rust/Poem server emitting an OpenAPI contract, native clients per platform, deployed to the homelab | `someproduct` | all of them |
| **[Library](archetypes/library.md)** | A shared `lib*` crate consumed by other repos over git | `libsomeproduct` + the `lib*` family | versioning (light), identifiers, testing |
| **[Orchestration](archetypes/orchestration.md)** | Deployment/infra-as-code for the homelab itself | homelab | deployment, identifiers, secrets split |
| **[CLI / service tool](archetypes/cli-tool.md)** | Standalone binary published to the registry with an install manifest | someproduct (CLI), courier | versioning, registry publishing, build info |

When a repo blends types (e.g. a product that also ships a CLI), apply each archetype to
the relevant subtree.

## The standards

| Topic | Doc | One-line rule |
|---|---|---|
| Versioning | [versioning.md](versioning.md) | Derived `MAJOR.MINOR.<commit count>`; tag is the only manual knob. |
| Identifiers | [identifiers.md](identifiers.md) | Reverse-domain `dev.thmsn.*`; inject via `libproduct`. |
| Build info | [build-info.md](build-info.md) | Embed git/build metadata via `libbuildinfo` when feasible. |
| Branding | [branding.md](branding.md) | One in-repo, code-as-truth generator produces all assets. |
| CI/CD | [ci-cd.md](ci-cd.md) | Self-hosted runners; build → GHCR/registry on `main`. |
| Self-hosted agents | [self-hosted-agents.md](self-hosted-agents.md) | Runners managed by agentutil; dev Mac for Apple builds. |
| Deployment | [deployment.md](deployment.md) | Komodo GitOps + Watchtower + Caddy; secrets out of git. |
| Registry publishing | [registry-publishing.md](registry-publishing.md) | Client apps ship via `apps.dev.thmsn.dev`. |
| Testing | [testing.md](testing.md) | >80% on services; don't unit-test UIs. |
| Platform UX | [platform-ux.md](platform-ux.md) | Native per OS, designed for the target OS. |
| Workflow | [workflow.md](workflow.md) | Commit to `main`; versions for debugging, not releases. |
| Skills & MCP | [skills-and-mcp.md](skills-and-mcp.md) | Shared skills/MCP live here; repos link them in. |

### Implementation standards (services & clients)

Deeper technical conventions, mostly for the full-stack-product archetype (some apply to
libraries/CLIs too):

| Topic | Doc | One-line rule |
|---|---|---|
| Service architecture | [service-architecture.md](service-architecture.md) | Workspace layered `core→db→engine→api`; poem-openapi; 10s graceful drain. |
| Data & persistence | [data-persistence.md](data-persistence.md) | sqlx + SQLite, `.sql` migrations, typed `Id<T>` (UUIDv7). |
| API contracts | [contracts.md](contracts.md) | Server emits OpenAPI/AsyncAPI; clients generated + drift-checked. |
| Error handling | [error-handling.md](error-handling.md) | Domain `Error`→HTTP; `liberror`+`thiserror`+`valuable` trinity. |
| Auth integration | [auth-integration.md](auth-integration.md) | Central auth via SDK; `me()` is live authority; grant-gated. |
| Configuration | [configuration.md](configuration.md) | libconfig `Loader`: defaults→TOML→shared→prefixed env. |
| Observability | [observability.md](observability.md) | `liblog`/OTLP; `/api/metrics`; log ids/timings, never bodies. |
| Security | [security.md](security.md) | Argon2, ChaCha20-Poly1305, CSP, SSRF/decompression caps. |
| Web & client arch | [web-architecture.md](web-architecture.md) | TanStack Query (server) + Zustand (UI); generated client; shared Kit. |
| Rust conventions | [rust-conventions.md](rust-conventions.md) | Edition 2024, clippy pedantic + allowlist, git deps. |
| `lib*` ecosystem | [lib-ecosystem.md](lib-ecosystem.md) | Solve cross-cutting concerns once in a shared `lib*`. |

## Shared tooling

Beyond docs, this repo ships reusable tooling so every repo gets it through the same
submodule:

- **`skills/`** — Claude Code skills, linked into a repo's `.claude/skills/`.
- **`mcp/`** — custom MCP servers, referenced from a repo's `.mcp.json`.

See [skills-and-mcp.md](skills-and-mcp.md).

> Status: complete first draft. All standards, all four archetypes, skills-and-mcp, and the
> copyable templates are written and cross-linked. Open for iteration.
