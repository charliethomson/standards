---
name: thmsn-new-component
description: Add a new component to an existing product the standard way — a service/binary (crate + identifier + Dockerfile + build workflow + deploy wiring) or a client surface (ios/macos/winui/web). Use when adding a new service, binary, daemon, CLI, or app target to a repo already on the standards.
---

# thmsn-new-component — add a service or client surface

Grow an existing product without drifting from the standards. Confirm the **component name and
kind** first, then scaffold from `standards/templates/` and wire it in.

Read `.standards.conf` for `PRODUCT` / `PRODUCT_UPPER`. A component is named
`dev.thmsn.$PRODUCT.<component>` — a service (`server`, `worker`, `cli`, `<name>`) or an app
surface (`ios`, `macos`, `winui`, `web`). See [identifiers](standards/docs/identifiers.md).

## Service / binary component

1. **Crate** — add a member to the `server/` workspace (or `bin/<component>/`), with deps
   pointing inward per [service-architecture](standards/docs/service-architecture.md).
2. **Version + identity** — `build.rs` from [templates/rust/build.rs](standards/templates/rust/build.rs);
   `product_name!("dev.thmsn.$PRODUCT.<component>")` in `main.rs`; clap
   `#[command(version = env!("APP_VERSION"))]`.
3. **Telemetry/config/errors** — wire `libconfig` `Loader`, `liblog` bootstrap, and the domain
   `Error`→`to_poem` mapping from the rust templates, per the configuration/observability/
   error-handling standards.
4. **Image** — `Dockerfile` from [templates/docker/Dockerfile.rust](standards/templates/docker/Dockerfile.rust)
   (`{{BIN}}`=the bin target); a `<component>.build.yml` workflow (component-first naming,
   [ci-cd](standards/docs/ci-cd.md)) modeled on `server.build.yml`.
5. **Deploy** — if it's a long-running service, add a service to `deploy/compose/prod.compose.yml`
   and the komodo stack; if it's a CLI/tool, add a `cli.release.yml` publishing to the registry
   ([registry-publishing](standards/docs/registry-publishing.md)).

## Client surface (ios / macos / winui / web)

1. **App** under `apps/<surface>/`, native UI per [platform-ux](standards/docs/platform-ux.md)
   (SwiftUI / WinUI / React) — designed for that OS, not ported.
2. **Generated client** from the committed `api/openapi.json` ([contracts](standards/docs/contracts.md)),
   sharing the `<Product>Kit` / `<Product>Core`; isolate any platform-only design system.
3. **Identifier** — bundle id `dev.thmsn.$PRODUCT.<surface>` (Apple `bundleIdPrefix`).
4. **CI** — add a per-surface job to `ci.yml` (build + codegen drift check) and a
   `<surface>.release.yml` publishing to the registry; branding icons for the new platform via
   `branding/build.py`.

## Finish

Fill placeholders from `.standards.conf`; copy from templates rather than reinventing. Re-run
`/thmsn-standards` on the new area to confirm it's on-standard. Leave staged; don't commit unless asked.
