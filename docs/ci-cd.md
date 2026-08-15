# CI/CD

## Rule

CI runs on the **self-hosted Woodpecker** server, on self-hosted agents
([self-hosted-agents.md](self-hosted-agents.md)). Pipelines live in **`.woodpecker/`**, one
file per deliverable surface, and split into two families:

- **`*.ci.yml`** — the gate. Build, lint (warnings = errors), test, coverage, and
  **codegen-drift checks**. Runs on every push to `main`, every PR, and on demand.
- **`*.build.yml` / `*.release.yml`** — delivery. Build images/artifacts and publish them,
  but **only on `main`** (PRs build to catch breakage, they don't push).

Shared shell used by more than one pipeline lives in **`scripts/ci/`**, not inlined in YAML.

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
      submodule_override:
        standards: https://github.com/charliethomson/standards.git
```

The `submodule_override` rewrites the SSH submodule URL to HTTPS so the clone reuses the
forge token instead of needing a deploy key.

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
  `RUN --mount=type=ssh` needs `--ssh default` on the build. Put the build itself in a shared
  `scripts/ci/docker-build.sh` rather than duplicating it per pipeline.
- **Keep the build context as narrow as the Dockerfile allows.** Use the repo root only when
  the build genuinely reads `.git` (e.g. `libbuildinfo` deriving the version itself); a
  Dockerfile taking `RELEASE_VERSION` as a build-arg does not.
- **Client apps → registry.** `{ios,macos,windows,cli}.release.yml` build signed/notarized
  artifacts and publish to `apps.dev.thmsn.dev`. See
  [registry-publishing.md](registry-publishing.md) — the publish composite action documented
  there is **GitHub-Actions-only**; a Woodpecker repo calls the registry API from a
  `scripts/ci/publish.sh` instead.
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
  `warnings|unmaintained|unsound|yanked`; **there is no severity threshold**. A severity gate
  therefore means `cargo audit --json` plus a filter on the advisory's CVSS severity, or
  `cargo-deny`'s advisories config. That filter is the one piece a first adopter has to write;
  it is deliberately not templated here because no repo has one yet to copy from.
- **Pin a recent `cargo audit`, and do not read its exit code as a finding count.**
  cargo-audit 0.21.2 **fails to parse the current advisory database at all** (it rejects
  CVSS 4.0 advisories, which the database now contains) and exits non-zero having audited
  nothing. A tool failure and a real advisory are the same exit code, so the reporting step
  must show its output rather than being trusted silently.
- Exceptions: `cargo audit --ignore RUSTSEC-YYYY-NNNN` (or `deny.toml`) and npm's overrides —
  each with the rationale and expiry alongside it.

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
| `macos_cert_p12`, `macos_cert_password`, `macos_notary_key_p8`, `macos_notary_key_id`, `macos_notary_issuer_id` | macOS signing + notarization |

## Checklist

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
      recorded, **expiring** exceptions.
- [ ] Local-backend (macOS/Windows) pipelines use `skip_clone: true` + a hand clone + an env script.
