---
name: thmsn-standards-init
description: Bootstrap a new/empty repo to a standards archetype — scaffold the full layout, workflows, deploy, branding, and config from the standards templates. Use when starting a new product/library/tool and wanting it on-standard from day one, or "set this repo up to the standards".
---

# thmsn-standards-init — scaffold a repo to an archetype

Stand up the skeleton for a new repo per its archetype, entirely from the standards templates
served by the `standards` MCP server (`get_template`). This creates **many files**: confirm the
plan with the user before writing, and never overwrite existing files (report skips).

## 1. Establish identity

Ensure `.standards.conf` exists. If not, run
`standards init --product <slug> --archetype <type> [--upper ENV] [--lib name]` (an existing
repo; it writes `.standards.conf` and the `Standards:` line in `AGENTS.md` without clobbering
anything). For a brand-new empty directory, `standards new <archetype> <product> [--lib name]
[--upper ENV]` does that plus the base scaffold; then use this skill for what remains.
If `standards` isn't on `PATH`, `standards doctor` is how a machine gets set up; say so and stop.

Read `.standards.conf` → `PRODUCT`, `PRODUCT_UPPER`, `LIB`, `ARCHETYPE`. These fill the
`{{PRODUCT}}` / `{{PRODUCT_UPPER}}` placeholders in every template.

## 2. Read the target shape

`get_doc("archetypes/<ARCHETYPE>")` → its **"Repo shape"** and **"Standards that
apply"** sections. That's the blueprint. Build exactly that, no more.

## 3. Scaffold from templates

Fetch each relevant template with `get_template("<path>", {"PRODUCT": …, "PRODUCT_UPPER": …,
…})` (path relative to the templates root, e.g. `rust/build.rs`,
`github-workflows/ci.yml`) and write the rendered body into place. The server fills the
placeholders you pass and lists any it left unfilled; fill or report those. By archetype:

- **full-stack-product** — `server/` Cargo workspace (`core`/`db`/`engine`/`api` per
  [service-architecture](https://standards.thmsn.dev/d/service-architecture)); `apps/` for the platforms you're
  shipping; `api/` for the OpenAPI contract; `branding/build.py`; `deploy/` (compose + Caddyfile
  + komodo sync); `.github/workflows/` (`ci.yml`, `server.build.yml`, `web.build.yml`);
  root `VERSIONING.md` + `AGENTS.md`; rust templates (`build.rs`, `tarpaulin.toml`, `config.rs`,
  `error.rs`, `observability.rs`, workspace lints). Entity identifiers are NOT scaffolded —
  depend on `libid` (`Id<T>`/`PublicId<T>`, see [lib-ecosystem](https://standards.thmsn.dev/d/lib-ecosystem)).
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

Pass `PRODUCT`, `PRODUCT_UPPER`, and `BIN`/`LOOPBACK_PORT` etc. as the template comments
direct; `get_template` reports any `{{KEY}}` left unfilled. The full template list is at
https://standards.thmsn.dev/api/v1/templates. Rename workflows to the component-first scheme
([ci-cd naming](https://standards.thmsn.dev/d/ci-cd)).

## 4. Wire identity

Set `product_name!("dev.thmsn.$PRODUCT.<component>")` in each binary's `main.rs`; Apple
`bundleIdPrefix: dev.thmsn.$PRODUCT`; auth grants `dev.thmsn.$PRODUCT.{use,admin}` if it sits
behind auth. See [identifiers](https://standards.thmsn.dev/d/identifiers).

## 5. Report

Summarize created vs skipped, then the **manual follow-ups** the scaffold can't do: fill in real
code, register the Komodo sync + gateway ingress ([deployment](https://standards.thmsn.dev/d/deployment)),
generate branding assets (`python3 branding/build.py`), set repo secrets/variables. Leave
everything staged; don't commit unless asked.
