# CI/CD

## Rule

CI runs on the **self-hosted Woodpecker** server, on self-hosted agents
([self-hosted-agents.md](self-hosted-agents.md)). Pipelines live in **`.woodpecker/`**, one
file per deliverable surface, and split into two families:

- **`*.ci.yml`** — the gate. Build, lint (warnings = errors), test, coverage, and
  **codegen-drift checks**. Runs on every push to `main`, every PR, and on demand.
- **`*.build.yml` / `*.release.yml`** — delivery. Build images/artifacts and publish them,
  but **only on `main`** (PRs build to catch breakage, they don't push).

Pipelines are moving to **kiln**: Rust in the repo's **`.kiln/`** crate, which generates
the `.woodpecker/` files. Woodpecker stays the executor either way. A ported repo follows
[Pipelines as code: kiln](#pipelines-as-code-kiln) first; the YAML guidance in the rest of
this doc is the **pre-kiln path** for repos not yet ported, and still explains what the
generated files do.

Pre-kiln, shared shell used by more than one pipeline lives in **`scripts/ci/`**, not inlined
in YAML.

CI is also the **source of truth for the version** ([versioning.md](versioning.md)): it
computes `RELEASE_VERSION` once and threads it into every build.

## Which CI system: Woodpecker

**Woodpecker is the standard. GitHub Actions is legacy.** New repos get `.woodpecker/` and
never get a `.github/workflows/`. Repos still on Actions are grandfathered: don't open a
conversion task for its own sake, but when a change would add or materially alter a workflow,
convert the repo instead of extending the old one.

This is one sanctioned system, not two. The reasons are worth stating so nobody re-opens it:

- **The migration is one-way.** Six repos have converted and deleted `.github/` outright;
  none has converted back, and no repo has adopted Actions since the first conversion.
- **Two options would be the worst outcome available.** It doubles the templates and the
  agent-gotcha list, and — the real cost — it leaves whoever scaffolds the next repo to pick.
  A standard exists to remove that choice. The repos that converted each paid for the choice
  the old standard made for them.
- **Only Woodpecker satisfies the stated operating context.** [overview.md](overview.md)
  commits to self-hosted everything with "no reliance on external SaaS for the core loop".
  Actions on self-hosted runners still puts the scheduler and control plane on GitHub.
- **The differences are structural, not cosmetic** (see the mapping below), so a doc covering
  both would be twice as long and half as usable.

**What a converting repo should expect to lose or replace.** These are not oversights; each
is a GitHub feature with no Woodpecker equivalent:

| GitHub Actions | Woodpecker |
|---|---|
| `dorny/paths-filter` + `if: needs.changes.outputs.<area>` | native `when.path.include`, per pipeline. The `changes` job disappears. |
| Aggregate "CI passed" job | **nothing, and nothing is needed.** It existed only because a path-skipped required check stays pending forever on GitHub. Woodpecker reports each pipeline separately. Set required checks per pipeline. |
| `actions/upload-artifact` | **no artifact store.** Keep the exit code — that was the gate — and read failures from the step log. |
| `concurrency: cancel-in-progress` | not expressible in config; a **server/repo setting**. |
| `webfactory/ssh-agent` | `eval "$(ssh-agent -s)"` + a secret, in the step. |
| `arduino/setup-protoc`, `apt` workarounds | plain `apt-get`/`apk` — **step containers run as root**, so the no-passwordless-sudo problem is gone. |
| `Swatinem/rust-cache`, `cache-{from,to}: type=gha` | host **volume mounts** (below). Both are GitHub-cache clients with no meaning off GitHub. |
| Any `uses:` composite action | **cannot be called at all.** Replace with a script in `scripts/ci/`. |

[`templates/github-workflows/`](../templates/github-workflows/) is kept for the repos still on
Actions. **Do not scaffold from it.** This document is the reference for a new repo.

## Pipelines as code: kiln

> **The newest section here, and not yet exercised.** [`libci`](https://github.com/charliethomson/libci)
> has no consumer yet. The rules below are what it implements; confirm them on the first port.

[kiln](https://github.com/charliethomson/kiln) pipelines are Rust in a standalone `.kiln/`
crate. `cargo run --manifest-path .kiln/Cargo.toml -- emit woodpecker` writes
`.woodpecker/*.yml` from it; the files are committed, headed "Do not edit — regenerate", and
Woodpecker runs them unchanged. Gate them on drift (kiln's own `repo.checks` pipeline has the
job; it is not templated), because `emit` overwrites but never deletes, and an orphaned
workflow keeps running. There are three layers, each with a different owner:

| Layer | What | Import | Lives in |
|---|---|---|---|
| **kiln core** | `Pipeline`, `Job`, `Ctx`, triggers, `secrets!` | `kiln::prelude` | kiln (private) |
| **generic recipes** | cargo, buildx, bun/npm, git drift, Apple, Windows — ecosystem-generic only | `kiln::recipes::*` | kiln |
| **fleet recipes** | the version scheme, `apps.dev.thmsn.dev`, `ghcr.io/charliethomson`, `npm.dev.thmsn.dev`, the audit gate, the fleet's secret names, the service-repo templates | `libci::*` | [`libci`](https://github.com/charliethomson/libci) |

The split is kiln's rule, not taste: a bundled recipe must make sense to someone with no
connection to this fleet. Anything naming one of our hosts goes in `libci`. Dependency
URLs (kiln and `libci`, both over SSH) are in [rust-conventions.md](rust-conventions.md).

- **Service repos use `libci::templates`**, not copies. `BrandingCi`, `RustCi`,
  `ImageBuild` (`server.build` / `web.build`) and `WebCi` replace the `branding.ci`, `rust.ci`,
  `server.build`, `web.build` and `web.ci` YAML and the `scripts/ci/*.sh` behind them. What
  varied between repos is a parameter; a fix lands once and arrives with `cargo update`. A
  service repo's whole `.kiln/src/main.rs` is a registration list — see the
  [crate README](https://github.com/charliethomson/libci#readme). Every template's path filter includes `.kiln/**`.
- **Not templated yet:** `audit.ci`, the Apple/Windows/CLI release pipelines, and pipelines
  needing a service container (a database for repository tests). Write those as the repo's own
  kiln pipelines on top of `libci::{audit, registry, version}`, or keep the pre-kiln YAML.
- **The repo needs Woodpecker's _trusted_ (volumes) flag.** Generated workflows mount cache
  volumes under `/var/lib/woodpecker-cache/<repo>/kiln/` and, for image builds, the Docker
  socket. On an untrusted repo Woodpecker **refuses the whole workflow** — not just the cache —
  and the generated file's header says so.
- **Pull requests get no secrets unless the job opts in** with `.allow_on_pull_request()`.
  `.kiln/` runs from the PR branch, so the opt-in has to be a visible line in review. Only
  **read-only fetch credentials** take it: `ssh_key` in `rust.ci` and image PR builds,
  `thmsn_npm_token` in `web.ci`. Nothing release-shaped does — which is why `ImageBuild`
  returns two pipelines, `<name>` (push/manual, holds `ghcr_token`) and `<name>-pr` (build only).
- **Declare secrets once.** `Repo::secrets` replaces rather than accumulates, so pass a single
  `libci::secrets::declare(&[…])` of every group the repo uses plus its own table.

## Layout and naming

Pipelines are named **component-first, dot-namespaced**, so the `.woodpecker/` listing groups
by the surface each one delivers.

| Element | Rule | Example |
|---|---|---|
| **File** | `<component>.<action>[-<qualifier>].yml` | `rust.ci.yml`, `web.ci.yml`, `server.build.yml`, `macos.release.yml` |
| **Step id** | short verb | `build`, `coverage`, `release` |
| **Step `commands`** | the real commands, in gate order | — |

- **Component = the deliverable surface**: `server`/`rust`, `web`, `ios`, `macos`, `windows`,
  `cli`, `branding`. **Action** is the verb: `ci` (gate), `build` (image → GHCR), `release`
  (artifact → registry).
- **Workflow surface ≠ identifier component.** A surface names the OS/runner target
  (`windows`); the app's identifier component is `winui` — see [identifiers.md](identifiers.md).
- **There is no `ci.yml`.** Under Actions, `ci.yml` was the deliberate cross-component
  exception carrying per-area jobs, because one workflow gave one required status. Woodpecker
  has no aggregate, so the gate **splits per surface** and the exception disappears. Combine
  two surfaces into one pipeline only when splitting would duplicate expensive work (an Apple
  pipeline covering iOS + macOS, rather than cloning and running the shared test target twice).

## The gate (`*.ci.yml`)

Per surface, at minimum:

- **server / rust** — `cargo fmt --all -- --check` **first** (it needs no compile, so a
  formatting failure costs seconds), then `cargo build --workspace --all-targets`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, then the
  **OpenAPI drift check** (below).
- **coverage** — `cargo tarpaulin --engine llvm --out Xml`, threshold from `tarpaulin.toml`
  (**fail-under 80**). See [testing.md](testing.md). `--engine llvm` is required in a
  container: the default ptrace engine cannot disable ASLR without extra trust.
- **web** — `npm ci`, **`codegen:check`**, `typecheck`, `lint` (max-warnings 0), unit tests,
  then **Playwright e2e against a mocked server**. Use the
  `mcr.microsoft.com/playwright:vX.Y.Z-noble` image, **pinned to the `@playwright/test`
  version in `package-lock.json`** — it already carries the browser and its system libraries,
  and a version mismatch makes Playwright re-download the browser anyway.
- **apple / windows** — regenerate the API client and `git diff --exit-code` (drift check),
  run the core unit suite, build the app.
- **branding** — regenerate from the palette and `git diff --exit-code`, **plus a check for
  untracked output** (`git status --porcelain --untracked-files=all`). The untracked half is
  the one that catches a generator learning to emit a new file, and the one that is easy to
  drop by accident.

**Codegen drift is a first-class gate.** The server emits the OpenAPI contract; every client
is generated from it and CI fails if regeneration produces a diff. This is what keeps the
"contract = `api/openapi.json`" rule honest.

> **The drift step must not be able to hang.** The usual form is
> `cargo run -q --bin api -- openapi > ../api/openapi.json && git diff --exit-code`. That
> assumes the binary *parses that argument*. A server that ignores argv instead **starts and
> blocks** until the pipeline deadline kills it — a worse failure than a red step, because the
> log explains nothing. Verify the subcommand exists before adding the step.

## Triggers and path filters

Every pipeline declares its own triggers, and the path filter is shared between the `push` and
`pull_request` entries with a YAML anchor so the two cannot drift apart:

```yaml
when:
  - event: manual                    # always keep a hand-run path
  - event: push
    branch: main
    path: &rust_paths
      include:
        - "server/**"
        - "scripts/ci/**"
        - ".woodpecker/rust.ci.yml"
  - event: pull_request
    path: *rust_paths
```

- A pipeline's filter **always includes its own file and any `scripts/ci/` it calls** —
  otherwise a change to the pipeline cannot test itself.
- A change to the **OpenAPI spec** re-runs *every* client pipeline; their generated clients
  must stay in sync.
- Keep `event: manual`. It is the only way to re-run a pipeline whose paths did not change.

## Agents, labels, and clone

**Always set `labels:`.** An unlabelled pipeline can be scheduled onto any agent, including a
**local-backend** agent, which treats `image:` as a *host executable* rather than a container.

| Label | Agent | Use for |
|---|---|---|
| `platform: linux/amd64` | the Linux Docker agents | everything, by default |
| `os: macos` | the dev Mac | Apple builds, notarization, byte-exact rasterisation |
| `os: windows` | the Windows agent | Windows builds and releases |

`platform` is Woodpecker's built-in auto-advertised label. The `os` labels are set **on the
agent, by hand** — the macOS and Windows agents are hand-managed and have no checked-in
configuration, so confirm the value with the agent rather than assuming it.

The macOS and Windows agents run the **local backend**, which changes three things and
silently breaks pipelines that ignore them:

1. **`skip_clone: true` and clone by hand.** A `clone:` block naming an image cannot run, and
   Woodpecker's fallback adds the remote with an empty URL — every run dies on
   `fatal: 'origin' does not appear to be a git repository`, long before the build.
2. **`image:` names the host shell** — `bash` on macOS, `powershell` on Windows — not a
   container. `directory:` is ignored, so `cd` explicitly.
3. **The step inherits the agent process's environment**, which is not a login shell — `HOME`
   and `PATH` are both wrong, so Homebrew tools are not found, and a throwaway `HOME` breaks
   keychains and every cache. Fix it in a `scripts/ci/macos-env.sh` (or `windows-env.ps1`)
   sourced at the top of the step.

**If a pipeline's label does not match any registered agent it never schedules, and it fails
silently rather than red.** That is the most likely thing to go wrong on a repo's first run.

For Docker-agent pipelines, configure the clone explicitly when the version depends on it:

```yaml
clone:
  git:
    image: woodpeckerci/plugin-git
    settings:
      partial: false            # full history …
      tags: true                # … and tags, or version derivation is wrong
```

**Legacy:** pipelines in repos that still vendor the `standards/` submodule carry a
`submodule_override` entry for `standards` under these settings, which rewrites the SSH
submodule URL to HTTPS so the clone reuses the forge token. Don't add it to new pipelines;
standards come from the server now. Remove it when migrating a repo
([migration](migration.md#3-remove-ci-submodule-handling-by-hand)).

## Caching

The pipeline workspace is fresh every run, so **without a cache every run is a cold compile**.
Persist caches as host volume mounts:

```yaml
volumes:
  - /var/lib/woodpecker-cache/<product>/cargo-registry:/usr/local/cargo/registry
  - /var/lib/woodpecker-cache/<product>/cargo-git:/usr/local/cargo/git
  - /var/lib/woodpecker-cache/<product>/target-build:/cache/target-build
  - /var/lib/woodpecker-cache/<product>/target-coverage:/cache/target-coverage
```

- **The repo must be marked _trusted_ in Woodpecker** before any host volume mount works —
  both these caches and the `/var/run/docker.sock` mount used by the image builds. It is a
  server-side per-repo setting with no in-repo representation, so it is invisible from the
  code and is the first thing to check when caching "does nothing".
- **Give the plain build and coverage separate `CARGO_TARGET_DIR`s.** Tarpaulin's instrumented
  build otherwise invalidates the plain cache on every alternating run.
- **Bound the cache.** Cargo target dirs never shrink, and an unbounded one has taken an agent
  to 100% disk more than once — where it surfaces as truncated apt downloads and a
  `GPG error … invalid signature` that points nowhere near the cause. Call a shared
  `scripts/ci/cache-guard.sh <dir> <max-gb> <min-free-gb>` first in the step: it wipes the
  cache when it exceeds the cap **or** when the filesystem is below the free-space floor,
  because a small cache does not help if something else filled the disk.
- **Size the cap per repo; do not copy another repo's numbers.** The free-space floor is the
  one that actually matters.

## Private dependencies

Private crates (`auth-sdk`, the `lib*` family) are fetched over **SSH**, so every Rust step
needs the deploy key and libgit2 routed through the git CLI:

```yaml
environment:
  CARGO_NET_GIT_FETCH_WITH_CLI: "true"
  GIT_SSH_COMMAND: "ssh -o StrictHostKeyChecking=accept-new"
  SSH_KEY:
    from_secret: ssh_key
commands:
  - eval "$(ssh-agent -s)"
  - printf '%s\n' "$SSH_KEY" | ssh-add -
```

Use `ssh://` URLs for private repos and HTTPS for public ones
([rust-conventions.md](rust-conventions.md)).

## Delivery (`*.build` / `*.release`)

- **Services → GHCR.** `server.build.yml` / `web.build.yml` build the image and push
  `ghcr.io/<owner>/<product>-{api,web}:main`, gated on the event not being a `pull_request`.
  `RELEASE_VERSION` is passed as a `--build-arg`. A Dockerfile that fetches private crates via
  `RUN --mount=type=ssh` needs `--ssh default` on the build. On kiln this is
  `libci::templates::ImageBuild` over `libci::images::GhcrImage`; pre-kiln, put the
  build in a shared `scripts/ci/docker-build.sh` rather than duplicating it per pipeline.
- **Keep the build context as narrow as the Dockerfile allows.** Use the repo root only when
  the build genuinely reads `.git` (e.g. `libbuildinfo` deriving the version itself); a
  Dockerfile taking `RELEASE_VERSION` as a build-arg does not.
- **Client apps → registry.** `{ios,macos,windows,cli}.release.yml` build signed/notarized
  artifacts and publish to `apps.dev.thmsn.dev`. See
  [registry-publishing.md](registry-publishing.md). On kiln, publish with
  `libci::registry::AppRegistry`; the composite action documented there is
  **GitHub-Actions-only**, and a pre-kiln Woodpecker repo's `scripts/ci/publish.sh` is not to be
  copied further.
- **Do not keep a release trigger the pipeline cannot honour.** If the publish path is not
  ported yet, make the pipeline `event: manual` and say so in the file. A tag firing a release
  that fails is bad; one that goes green without releasing is worse.

## Dependency audit

> **New in this revision, and the least battle-tested section here.** The policy below is the
> intended standard; the Rust *mechanism* is only partly expressible with today's tooling, and
> that is called out rather than papered over.

Dependency advisories are checked in CI, on a schedule, and triaged against a clock. Without
the schedule, an advisory filed against code that nobody has touched is never surfaced at all.

**The policy:**

- **`*.ci.yml` runs an audit on every gate run**, non-blocking for advisories generally,
  **blocking on `critical`**. Rust audits `Cargo.lock`; web audits `package-lock.json`.
- **A weekly scheduled run** audits `main` on its own, so a new advisory against *unchanged*
  code still surfaces. This is a Woodpecker **cron**, a server-side per-repo object; the
  pipeline opts in with `event: cron`.
- **Triage has a named owner and a clock**: **critical → triaged within 24h**; high → the next
  planned cycle; everything else → best effort. "Triaged" means upgraded, mitigated, or
  recorded as an exception — not read and forgotten.
- **Exceptions are recorded and expire.** An accepted advisory is ignored *by id*, in-repo,
  with a rationale and an **expiry date**. When it expires the gate goes red again on purpose.
  An exception with no expiry is silence, which is the thing this section exists to prevent.

**The CI shape** — two steps, because "non-blocking except critical" is two different exit
codes over the same data:

```yaml
when:
  - event: manual
  - event: cron
    cron: weekly-audit
  - event: push
    branch: main
    path: &audit_paths
      include:
        - "server/Cargo.lock"
        - "apps/web/package-lock.json"
        - ".woodpecker/audit.ci.yml"
  - event: pull_request
    path: *audit_paths

steps:
  advisories:            # visibility: reports everything, never fails the pipeline
    failure: ignore
    commands: [ … ]

  critical:              # the gate: exits non-zero on critical only
    commands: [ … ]
```

**No repo in the fleet runs a cron pipeline today**, so this is the first. The `event: cron` +
`cron: <name>` and `failure: ignore` syntax above lints clean under
`woodpecker-cli lint --strict` (3.16.0), but the **schedule itself is a server-side per-repo
object** someone has to create in the UI, named to match. Until it exists the pipeline simply
never fires on a schedule — silently, like a mismatched label.

**Tooling, and one sharp edge:**

- **Web — expressible directly.** `npm audit --audit-level=critical` is the blocking step;
  plain `npm audit` (or `--audit-level=high`) under `failure: ignore` is the reporting step.
- **Rust — not directly expressible.** `cargo audit`'s `--deny` takes
  `warnings|unmaintained|unsound|yanked`; **there is no severity threshold**, so a severity gate
  means `cargo audit --json` plus a filter on CVSS. On kiln that filter is
  `libci::audit::AuditGate` (below); pre-kiln, it is a repo-local script.
- **Pin a recent `cargo audit`, and do not read its exit code as a finding count.**
  cargo-audit 0.21.2 **fails to parse the current advisory database at all** (it rejects
  CVSS 4.0 advisories, which the database now contains) and exits non-zero having audited
  nothing. A tool failure and a real advisory are the same exit code, so the reporting step
  must show its output rather than being trusted silently.
- Pre-kiln exceptions: `cargo audit --ignore RUSTSEC-YYYY-NNNN` (or `deny.toml`) and npm's
  overrides — each with the rationale and expiry alongside it.

**On kiln: `libci::audit::AuditGate`** (feature `audit`). kiln's recipes produce the
reports (`CargoAudit::run(ctx)…json()`, `Npm::audit(ctx)…report_to(…)`, `Bun::audit(ctx)…`);
the gate is the policy over them, and one step replaces the two above — it logs every finding
and fails only on a blocking one:

```rust
AuditGate::new()
    .exceptions("audit-exceptions.json")
    .cargo("server", server_json)
    .npm("web", web_json)
    .run(ctx)?;
```

- **Critical** is a CVSS 3.x base score ≥ 9.0, or a `critical` severity label (bun always; npm
  when the advisory has no vector; a cargo advisory with no vector is unscored). A CVSS 4.0
  vector blocks only in the worst-case shape (network, low complexity, no attack requirements,
  no privileges, no interaction, High impact on the vulnerable system); any other 4.0 vector is
  reported unscored for manual triage and never blocks.
- **The gate fails** on a critical finding with no live exception, **or on any expired
  exception** — whether or not the advisory still appears. An empty or non-JSON report is an
  error, so a tool that failed cannot read as clean.
- **The exceptions file** is a JSON list, at a path the repo passes to `.exceptions(…)`
  (relative to the workspace; a missing file means no exceptions):

  ```json
  [
    {
      "id": "RUSTSEC-2025-0001",
      "reason": "only reachable through the CLI's offline mode; upgrade blocked on upstream",
      "expires": "2026-12-31"
    }
  ]
  ```

  All three fields are required and non-empty. `id` is the advisory id as reported
  (`RUSTSEC-…`, `GHSA-…`). `expires` is `YYYY-MM-DD`, the **last day it holds**, compared with
  today in UTC. A missing field or a malformed date fails the gate. An exception covers only a
  critical finding; everything else was never going to block.
- **The weekly run** is `.on(Schedule::cron("…"))`. The emitted `event: cron` names the
  pipeline, so the Woodpecker UI schedule must be **named after the pipeline** — the expression
  in code is recorded as a comment, not honoured.

## Secrets

Referenced as `from_secret:` and **provisioned by hand in the Woodpecker UI** — there is no
IaC for them, and none of repo enablement, the trusted flag, or secrets is represented in the
repo. Record what a repo needs in its `AGENTS.md`, because nothing else will. The recurring
set:

| Secret | Used by |
|---|---|
| `ssh_key` | every Rust pipeline and any hand-clone step (private crates, private submodules) |
| `ghcr_token` | `*.build.yml` — needs `write:packages` |
| `thmsn_npm_token` | web pipelines, for `@thmsn/ui` from the internal npm registry |
| `registry_token` | `*.release.yml`, publishing to `apps.dev.thmsn.dev` |
| `registry_repo_token` | kiln release jobs: read-only clone of `charliethomson/registry`'s publish client |
| `macos_cert_p12`, `macos_cert_password`, `macos_notary_key_p8`, `macos_notary_key_id`, `macos_notary_issuer_id` | macOS signing + notarization |
| `ios_cert_p12`, `ios_cert_password`, `ios_adhoc_profile` | iOS ad-hoc signing |

On kiln these keys are `libci::secrets` types (`GhcrToken = "ghcr_token"`, …). The keys
are the contract with the secret store: renaming one is an operations change in every repo.

## Checklist

- [ ] Ported repos: pipelines in `.kiln/`; `.woodpecker/` generated by `emit woodpecker`, never
      hand-edited, and drift-checked; service pipelines from `libci::templates`, not
      copied YAML or `scripts/ci/`.
- [ ] Ported repos: Woodpecker's trusted (volumes) flag set; only read-only fetch credentials
      use `.allow_on_pull_request()`.
- [ ] Pipelines live in `.woodpecker/`, named `<component>.<action>.yml`; no `.github/workflows/`.
- [ ] Each pipeline sets `labels:` — `platform: linux/amd64`, or `os: macos` / `os: windows`.
- [ ] `when:` covers `manual` + `push: main` + `pull_request`, with the path filter shared by anchor.
- [ ] Path filters include the pipeline's own file and any `scripts/ci/` it calls.
- [ ] `cargo fmt --check` first; clippy/lint warnings-as-errors; tests run; tarpaulin enforces 80%.
- [ ] Every generated client has a drift check (`git diff --exit-code`), and it cannot hang.
- [ ] Caches are host volumes with the trusted-volumes grant, bounded by `cache-guard.sh`,
      with separate target dirs for build and coverage.
- [ ] Private crates: `ssh_key` secret + `CARGO_NET_GIT_FETCH_WITH_CLI` + `GIT_SSH_COMMAND`.
- [ ] `*.build` push images only on `main`; `RELEASE_VERSION` computed once and passed in.
- [ ] Release pipelines whose publish path is not ported are `event: manual`, and say so.
- [ ] Dependency audit runs in the gate and on a **weekly cron**, blocking on critical, with
      recorded, **expiring** exceptions (kiln: `AuditGate` + its exceptions file).
- [ ] Local-backend (macOS/Windows) pipelines use `skip_clone: true` + a hand clone + an env script.
