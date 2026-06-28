# Changelog

Notable changes to the **standards** repo. This is a docs/reference repo vendored as a
submodule, so entries are human-facing and *not* tied to the derived version scheme in
[`docs/versioning.md`](docs/versioning.md) (that governs product/binary builds, not this repo).
Format loosely follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

First draft — nothing tagged yet; everything lives here until a `v0.1` cut.

### Added

- **Entrypoint & index** — `README.md`, `AGENTS.md` (agent entrypoint), `docs/overview.md`
  (operating context + archetype picker + index).
- **Archetypes** — `docs/archetypes/`: full-stack-product, library, orchestration, cli-tool,
  each mapping which standards apply and how strictly.
- **Foundational standards** — versioning, identifiers, build-info, ci-cd, self-hosted-agents,
  deployment, registry-publishing, testing, platform-ux, workflow, branding, skills-and-mcp.
- **Implementation standards** (services & clients) — service-architecture, data-persistence,
  contracts, error-handling, auth-integration, configuration, observability, security,
  web-architecture, rust-conventions, lib-ecosystem.
- **Copyable templates** (`templates/`) — `VERSIONING.md`; `rust/` (`build.rs`,
  `tarpaulin.toml`, `id.rs`, `error.rs`, `config.rs`, `observability.rs`,
  `Cargo-workspace-lints.toml`); `github-workflows/` (`ci.yml`, `server.build.yml`,
  `web.build.yml`); `docker/Dockerfile.rust`; `deploy/` (compose, Caddyfile, komodo sync);
  `branding/build.py`; the consuming-repo `AGENTS.md` stub; `link-standards.sh`; `mcp.json`.
- **Shared tooling scaffolding** — `skills/` and `mcp/` with conventions; `link-standards.sh`
  symlinks shared skills into `.claude/skills/` and scaffolds `.mcp.json`.
- **Shared skills** — six `thmsn-*` skills: `thmsn-standards` (audit/fix the repo against its
  applicable standards), `-review` (diff-scoped check of just your changes), `-init` (scaffold a
  new repo to an archetype), `thmsn-new-component` (add a service or client surface on-standard),
  `-sync` (update the submodule + re-audit), and `-contribute` (author + push a genericized
  standards change). Consuming repos get them via the submodule + `link-standards.sh`.
- **Integration CLI** (`bin/standards`) — `install` (wire the submodule into a repo), `sync`
  (pull upstream + show the changelog + re-link + stage the bump), `contribute` (genericize a
  repo's identifiers and push the change upstream), and `lint` (verify your edits carry none of
  your product's own names). Identity is read from a consumer-side `.standards.conf` — **no
  product names are stored in this repo**.

### Changed

- Replaced the reconstructed `templates/VERSIONING.md` with the byte-identical fleet canonical.
- Standardized GitHub Actions naming on **Scheme B** (component-first dot-namespaced):
  renamed `build-server.yml`/`build-web.yml` → `server.build.yml`/`web.build.yml`; added the
  "Workflow naming" section to `docs/ci-cd.md`; updated all references.
- `docs/configuration.md`: **`libconfig`** is now the fleet config tool for services too — after
  landing the `libconfig` shared-env layer + `Loader` file source (`2173a3a`) and the `libpath`
  base-dir override + no-create (`251628a`). Removed the interim `figment` framing; the
  `config.rs` template now uses `Loader::path(..).env_prefix(..).shared_env(..)`.
- `docs/identifiers.md`: reframed the segment after `<product>` as a **`<component>`** (a
  service like `server`/`courier`/`cli` or an app surface like `ios`/`macos`/`winui`/`web`),
  with grants documented as a parallel axis and an explicit grammar — replacing the looser
  `<platform|grant>` shorthand.
- **Anonymized examples** across docs + templates: product/library names → `dev.thmsn.someproduct`
  / `libsomeproduct`, and the sample domain vocabulary → neutral terms (resource/model names, an
  external-adapter example, a metric name) so the docs are fully product-neutral. The real `lib*`
  toolchain and shared-infrastructure names are kept — they're the actual platform, not examples.

### Notes

- Standards are written **canonically** (the rule as target state), per the deep-docs decision.
- Known external follow-up: a reference repo's `VERSIONING.md` has drifted from the fleet canonical.
