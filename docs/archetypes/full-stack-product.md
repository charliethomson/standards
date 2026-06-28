# Archetype: Full-stack product

A user-facing product: a Rust/Poem server that emits an OpenAPI contract, with native
clients per platform, deployed to the homelab. The heavyweight archetype — **every
standard applies**.

Reference: `someproduct` — a Rust/Poem server emitting an OpenAPI contract, with native clients.

## Three sources of truth

This is the organizing principle. Each concern has exactly one home:

- **Behaviour → the server.** One implementation; every client calls it. No business logic
  is reimplemented per platform.
- **Contract → `api/openapi.json`.** Emitted by the server, committed, and used to
  generate typed clients. CI fails if a client's generated code drifts from the spec.
- **UX → the web client.** The reference design; native clients port features from it
  (not the other way round). See [platform-ux.md](../platform-ux.md).

## Repo shape

```
<product>/
├── server/                 Rust Cargo workspace (core, db, api, …); emits OpenAPI
├── apps/
│   ├── web/                React + Vite + Tailwind + shadcn (reference UX)
│   ├── apple/              SwiftUI iOS + macOS, shared SPM <Product>Kit
│   └── windows/            WinUI 3 + shared testable core
├── api/                    openapi.json (+ asyncapi) — generated, committed
├── branding/               code-as-truth asset generator (see branding.md)
├── deploy/                 Komodo sync TOML + compose + Caddyfile
├── .github/workflows/      *.build (→ GHCR) + *.release (→ registry); see ci-cd.md naming
├── AGENTS.md               stub → standards/
├── VERSIONING.md           copied from standards/templates
└── standards/              this submodule
```

## Standards that apply

| Standard | How it applies here |
|---|---|
| [Versioning](../versioning.md) | Derived `MAJOR.MINOR.<count>`; computed in CI, consumed by server/Docker/Xcode/web. |
| [Identifiers](../identifiers.md) | `dev.thmsn.<product>` root; per-platform bundle ids; auth grants `.{read,write,admin}`. |
| [Build info](../build-info.md) | `libbuildinfo` in the server (and shipped binaries); exposed via `/version`. |
| [Branding](../branding.md) | In-repo generator produces all icons/tokens for every platform. |
| [CI/CD](../ci-cd.md) | Self-hosted runners; `*.build` → GHCR `:main`; `*.release` → registry. Codegen drift + coverage gates. |
| [Self-hosted agents](../self-hosted-agents.md) | Linux/macOS runners via agentutil; dev Mac for Apple notarized builds. |
| [Deployment](../deployment.md) | Komodo GitOps + Watchtower + Caddy; hostname `<product>.dev.thmsn.dev`; secrets in Komodo UI. |
| [Registry publishing](../registry-publishing.md) | Client apps (iOS/macOS/Windows) ship to `apps.dev.thmsn.dev`. |
| [Testing](../testing.md) | >80% tarpaulin on the server; vitest + hermetic Playwright on web; thin/none on native UI. |
| [Platform UX](../platform-ux.md) | Native per OS; web is the reference, ported natively. |
| [Workflow](../workflow.md) | Commit to `main`; continuous deploy. |
| [Service architecture](../service-architecture.md) | Workspace `core→db→engine→api`; poem-openapi; 10s drain. |
| [Data & persistence](../data-persistence.md) | sqlx + SQLite, `.sql` migrations, typed `Id<T>`. |
| [Contracts](../contracts.md) | Emit OpenAPI/AsyncAPI; generate + drift-check clients. |
| [Error handling](../error-handling.md) | Domain `Error`→HTTP; `liberror`/`valuable` trinity. |
| [Auth integration](../auth-integration.md) | Central auth SDK; `me()` live authority; grant-gated. |
| [Configuration](../configuration.md) | `libconfig` `Loader`; `<PRODUCT>_` + bare shared env. |
| [Observability](../observability.md) | liblog/OTLP; `/api/metrics`; ids/timings only. |
| [Security](../security.md) | Argon2, ChaCha20-Poly1305, CSP, SSRF/decompression caps. |
| [Web & client arch](../web-architecture.md) | TanStack Query + Zustand; generated client; shared Kit. |
| [Rust conventions](../rust-conventions.md) | Edition 2024, clippy pedantic, git deps. |
| [`lib*` ecosystem](../lib-ecosystem.md) | Reuse shared `lib*` for cross-cutting concerns. |

## Auth integration

Products that sit behind central auth register an app id `dev.thmsn.<product>` and a
`Grants` enum (`Read`/`Write`/`Admin`) via the auth SDK's `#[derive(App)]`, registered
idempotently on startup. Grants are `dev.thmsn.<product>.{read,write,admin}`.

## Checklist

- [ ] Server emits `api/openapi.json`; clients are generated from it with a CI drift check.
- [ ] No business logic duplicated outside the server.
- [ ] Native clients exist per target OS, designed for that OS (not ported wrappers).
- [ ] Branding generator present and run; no hand-edited icon assets.
- [ ] `deploy/` has Komodo sync TOML + compose + Caddyfile; secrets only in Komodo UI.
- [ ] `*.build`/`*.release` workflows on self-hosted runners; version computed once.
- [ ] Server ≥80% coverage; web has hermetic e2e; native UI not unit-tested.
- [ ] Root `AGENTS.md` stub + `VERSIONING.md` copied from `standards/templates`.
