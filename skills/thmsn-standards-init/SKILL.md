---
name: thmsn-standards-init
description: Bootstrap a new/empty repo to a standards archetype — scaffold the full layout, workflows, deploy, branding, and config from the standards templates. Use when starting a new product/library/tool and wanting it on-standard from day one, or "set this repo up to the standards".
---

# thmsn-standards-init — scaffold a repo to an archetype

Stand up the skeleton for a new repo per its archetype, entirely from
`standards/templates/`. This creates **many files** — confirm the plan with the user before
writing, and never overwrite existing files (report skips).

## 1. Establish identity

Ensure the submodule is installed and `.standards.conf` exists. If not:
`git submodule add git@github.com:charliethomson/standards standards` then
`standards/bin/standards install --product <slug> --archetype <type> [--upper ENV] [--lib name]`.

Read `.standards.conf` → `PRODUCT`, `PRODUCT_UPPER`, `LIB`, `ARCHETYPE`. These fill the
`{{PRODUCT}}` / `{{PRODUCT_UPPER}}` placeholders in every template.

## 2. Read the target shape

Open `standards/docs/archetypes/<ARCHETYPE>.md` → its **"Repo shape"** and **"Standards that
apply"** sections. That's the blueprint. Build exactly that, no more.

## 3. Scaffold from templates

Copy the relevant `standards/templates/` files into place, filling placeholders. By archetype:

- **full-stack-product** — `server/` Cargo workspace (`core`/`db`/`engine`/`api` per
  [service-architecture](standards/docs/service-architecture.md)); `apps/` for the platforms you're
  shipping; `api/` for the OpenAPI contract; `branding/build.py`; `deploy/` (compose + Caddyfile
  + komodo sync); `.github/workflows/` (`ci.yml`, `server.build.yml`, `web.build.yml`);
  root `VERSIONING.md` + `AGENTS.md`; rust templates (`build.rs`, `tarpaulin.toml`, `config.rs`,
  `error.rs`, `id.rs`, `observability.rs`, workspace lints).
- **library** — Cargo workspace, `lib<name>/`, `examples/`, `.cargo/config.toml` if it needs
  `tracing`/private deps, `README.md`, `VERSIONING.md` (placeholder version). CI only if it ships
  an artifact.
- **cli-tool** — the binary crate + `build.rs`, `completions/`, a `cli.release.yml`, root
  `VERSIONING.md`.
- **orchestration** — one dir per stack, `komodo/sync/*.toml`, `docs/HOSTS.md` + `RUNBOOK.md`.
- **vendored-app** — the third-party app as an `upstream/` submodule (pinned), `scripts/` for
  self-verifying build-time adjustments, a `<platform>.release.yml` that builds/signs/publishes,
  root `AGENTS.md` spelling out the à-la-carte overrides. No source, no rebrand, no `VERSIONING.md`
  (ship upstream's version).

Replace `{{PRODUCT}}`→`$PRODUCT`, `{{PRODUCT_UPPER}}`→`$PRODUCT_UPPER`, `{{BIN}}`/`{{LOOPBACK_PORT}}`
etc. as the template comments direct. Rename workflows to the component-first scheme
([ci-cd naming](standards/docs/ci-cd.md)).

## 4. Wire identity

Set `product_name!("dev.thmsn.$PRODUCT.<component>")` in each binary's `main.rs`; Apple
`bundleIdPrefix: dev.thmsn.$PRODUCT`; auth grants `dev.thmsn.$PRODUCT.{use,admin}` if it sits
behind auth. See [identifiers](standards/docs/identifiers.md).

## 5. Report

Summarize created vs skipped, then the **manual follow-ups** the scaffold can't do: fill in real
code, register the Komodo sync + gateway ingress ([deployment](standards/docs/deployment.md)),
generate branding assets (`python3 branding/build.py`), set repo secrets/variables. Leave
everything staged; don't commit unless asked.
