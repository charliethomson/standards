# Glossary

Terms, tools, and infrastructure referenced across the standards, defined once. Real shared
infrastructure and the `lib*` toolchain are named here because they *are* the platform; the
example placeholders (`someproduct`, `libsomeproduct`, `worker`) stand in for whatever you're
actually building.

## Conventions & placeholders

- **archetype** — which kind of repo this is: **full-stack-product**, **library**,
  **orchestration**, or **cli-tool**. Determines which standards apply. See
  [archetypes/](archetypes/).
- **`someproduct` / `libsomeproduct`** — the generic placeholders every example uses in place of
  a real product/library name. Never a real product name appears in this repo.
- **`<component>`** — the segment after the product in an identifier: a **service** (`server`,
  `worker`, `cli`, `<service_name>`) or an **app surface** (`ios`, `macos`, `winui`, `web`). See
  [identifiers.md](identifiers.md).
- **app surface** — a client-platform target of a product (the web app, the iOS app, …).
- **`worker`** — the generic example name for a background companion service that hangs off the
  main `server` (stands in for whatever real sidecar you have).
- **Scheme B** — the workflow-naming convention: component-first, dot-namespaced
  (`server.build.yml`, `macos.release.yml`); `ci.yml` is the lone cross-cutting exception. See
  [ci-cd.md](ci-cd.md).
- **`{{PRODUCT}}` / `{{PRODUCT_UPPER}}` / `{{BIN}}`** — placeholders in `templates/` filled at
  install time from `.standards.conf`.

## Core concepts

- **derived version** — `MAJOR.MINOR.<commit count>`, computed from git, never stored. The tag
  is the only manual knob. See [versioning.md](versioning.md).
- **`RELEASE_VERSION` / `APP_VERSION`** — the CI-computed version env var, and the value baked
  into a binary via `build.rs` (read with `env!("APP_VERSION")`).
- **reverse-domain identifier** — `dev.thmsn.<product>[.<component>]`; the fleet-wide naming root.
  See [identifiers.md](identifiers.md).
- **grant** — an auth permission, `dev.thmsn.<product>.<grant>` (`use`/`read`/`write`/`admin`),
  declared with `#[derive(App)]`. See [auth-integration.md](auth-integration.md).
- **`AppId`** — the validated reverse-domain identifier type (lowercase, ≥2 segments).
- **`Id<T>`** — a typed UUIDv7 entity id; `Id<Order>` can't be passed where `Id<Item>` is
  expected. See [data-persistence.md](data-persistence.md).
- **the trinity** — `thiserror` + `liberror` + `valuable`: the derives that make an error type
  serializable, chain-preserving, and structured-loggable. See [error-handling.md](error-handling.md).
- **error key** — the stable reverse-DNS `$type` on a serializable error variant,
  `dev.thmsn.<root>.<area>.error.<kind>` — names exactly what failed so you can grep straight to
  the code path. Part of the reverse-domain identity family. See [error-handling.md](error-handling.md).
- **OTLP** — OpenTelemetry protocol; services export to the homelab collector with
  `service.name = dev.thmsn.<product>.<component>`. See [observability.md](observability.md).
- **codegen drift check** — CI fails if a generated client no longer matches the committed
  `api/openapi.json`. See [contracts.md](contracts.md).
- **three sources of truth** — behaviour = the server, contract = `api/openapi.json`, UX = the
  web client. See [archetypes/full-stack-product.md](archetypes/full-stack-product.md).
- **`Loader`** — `libconfig`'s builder for loading service config (`path`/`pure_env`/`module`)
  with the shared-env layer. See [configuration.md](configuration.md).

## Platform infrastructure (real, shared)

- **auth** — the central authentication service (TCP/protobuf SDK + REST). Products register
  their app + grants on boot and validate every request's JWT against it.
- **registry** — the self-hosted app registry at **`apps.dev.thmsn.dev`** where client apps and
  CLIs publish. **`dev.thmsn.apps`** is its AltStore source identifier (never changes). See
  [registry-publishing.md](registry-publishing.md).
- **agentutil** — manages the fleet of self-hosted GitHub Actions runners over SSH. See
  [self-hosted-agents.md](self-hosted-agents.md).
- **Komodo / Periphery** — the GitOps deployment control plane (Komodo Core) and its
  per-host agents (Periphery) that reconcile docker-compose stacks from git.
- **homelab** — the infra-as-code repo that deploys the whole fleet; the **`dev` box** is its
  primary host (`192.168.0.193`).
- **Watchtower** — auto-updates running containers when a new `:main` image is pushed to GHCR.
- **Caddy gateway** — the homelab reverse proxy that terminates TLS (Cloudflare DNS-01) and
  fronts each stack at `<product>.dev.thmsn.dev`. See [deployment.md](deployment.md).
- **GHCR** — GitHub Container Registry; where service images (`<product>-{api,web}:main`) live.
- **`LIBPATH_BASE_DIR`** — env var that redirects `libpath`'s config/log roots to a
  deploy-controlled path in a container.

## The `lib*` toolchain

Shared crates that solve cross-cutting concerns once. See [lib-ecosystem.md](lib-ecosystem.md).

- **liberror** — serializable error wrapper preserving the source chain (`AnyError`).
- **liblog** — the `tracing` + OpenTelemetry subscriber stack (`builder().…init()`).
- **libsignal** — cross-platform graceful shutdown on signals.
- **libconfig** — TOML + env config with mtime tracking and the `Loader` builder.
- **libpath** — platform app directories (config/logs/cache/data), keyed by product name.
- **libproduct** — the reverse-domain product descriptor and the `product_name!` macro.
- **libbuildinfo** — build metadata (git/OS/package) embedded at build time.
- **libcmd** — async subprocess execution with streaming + cancellation.
- **libwhich** — binary discovery on `PATH`.
- **libring** — fixed-size ring buffer for bounded history.
- **libfs** — tokio async filesystem helpers.

## Standards tooling

- **the `standards` submodule** — this repo, vendored into each consuming repo at `standards/`.
- **`.standards.conf`** — a consumer-side file (in *your* repo, not here) recording the product's
  identity (`PRODUCT`, `PRODUCT_UPPER`, `LIB`, `ARCHETYPE`) for the CLI and skills.
- **`bin/standards`** — the integration CLI: `install`, `sync`, `contribute`, `lint`.
- **`link-standards.sh`** — symlinks shared skills into `.claude/skills/` and scaffolds `.mcp.json`.
- **`thmsn-*` skills** — the shared Claude Code skills (audit, review, init, new-component, sync,
  contribute). See [skills-and-mcp.md](skills-and-mcp.md).
