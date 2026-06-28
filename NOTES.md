# Notes

Working notes for the standards repo — in-flight work, open threads, and the decision log.
Not part of the standards themselves; this is the scratchpad that keeps context between
sessions.

## Done

### libpath + libconfig — unify service config on `libconfig` ✅

**Landed** (task `task_1e109f7b`): `libpath` `251628a` (base-dir override + no-create) and
`libconfig` `2173a3a` (shared-env layer + pluggable `Loader` file source). The interim figment
framing is removed from [docs/configuration.md](docs/configuration.md); the config template now
uses `Loader`.

Shipped API:
- **libconfig** — `Loader::module(name)` (OS dir, write-back) / `Loader::path(p)` (deploy file,
  read-only, no mkdir) / `Loader::pure_env()`; `.env_prefix(..).shared_env([..]).load::<T>()`.
  Precedence defaults→TOML→shared→prefixed. `config!{}` gained a `shared_env` field. Old free
  `load`/`store`/`load_tracked` + desktop flow unchanged.
- **libpath** — `LIBPATH_BASE_DIR` env / `set_base_override()` redirect every root;
  `set_create_dirs(false)` + `*_no_create` variants. Also moves `liblog`'s log file off the OS dir.

## Open threads

- **`naming.md` reference** *(optional)* — a single doc gathering every naming convention
  (reverse-domain identifiers, env-var prefixes, GHCR image tags `<product>-{api,web}:main`,
  deployment hostnames `<service>.dev.thmsn.dev`, workflow names, branch = `main`) that
  currently lives spread across topic docs. Cross-links to the detail docs.
- **Seed the first shared MCP server** — `mcp/` holds only a README placeholder (`skills/`
  now has six `thmsn-*` skills).
- **Reference-repo `VERSIONING.md` drift** — one reference repo has diverged from the fleet
  canonical; re-sync when convenient (external code, not touched).

## Decision log

- **Config tooling** — unified on `libconfig` (services use `Loader`); the libpath + libconfig
  enhancements landed (see Done).
- **Workflow naming** — Scheme B (component-first dot-namespaced): `server.build.yml`,
  `macos.release.yml`; `ci.yml` is the lone cross-cutting exception. Lives in `docs/ci-cd.md`.
- **Branding** — code-as-truth generator (the chosen approach), extended to all platforms, at
  top-level `branding/`.
- **Skills** — standardized on `.claude/skills/` (shared skills linked from `standards/skills/`);
  `.agents/skills/` deprecated.
- **Submodule integration** — vendored at `standards/`; root `AGENTS.md` stub points agents in.
- **Doc format** — prose + copyable templates; each standard is rule → why → implementation →
  checklist.
- **Convergence treatment** — write standards canonically (target state), no per-repo divergence
  notes (the config interim is the deliberate exception, because it's actively being closed).
- **Archetypes** — full-stack-product, library, orchestration, cli-tool.
