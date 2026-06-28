# CI/CD

## Rule

CI runs on **self-hosted runners**, triggers on `push` to `main` and on `pull_request`,
and splits into two families:

- **`ci.yml`** — the gate. Build, lint (warnings = errors), test, coverage, and
  **codegen-drift checks**. Runs on every push/PR; a single aggregate "CI passed" job is
  the required status.
- **`*.build.yml` / `*.release.yml`** — delivery. Build images/artifacts and publish them,
  but **only on `main`** (PRs build to catch breakage, they don't push).

CI is also the **source of truth for the version** ([versioning.md](versioning.md)): it
computes `RELEASE_VERSION` once and threads it into every build.

## The gate (`ci.yml`)

Structure, faithfully from the reference repos:

1. **`changes` job** uses `dorny/paths-filter@v3` to detect which areas changed (server,
   web, apple, windows). Every downstream job is `if: needs.changes.outputs.<area>`, so a
   server-only change doesn't rebuild the clients. A change to the **OpenAPI spec**
   (`api/openapi.json`) re-runs *every* client (their generated clients must stay in sync);
   a change to `ci.yml` itself runs everything.
2. **Per-area jobs:**
   - **server** — `cargo build --workspace --all-targets`, `cargo clippy … -- -D warnings`,
     `cargo test --workspace`.
   - **coverage** — `cargo tarpaulin --out Xml`, threshold from `tarpaulin.toml`
     (**fail-under 80**). See [testing.md](testing.md).
   - **web** — `npm ci`, **`codegen:check`** (typed client must regenerate cleanly from the
     committed spec), `typecheck`, `lint` (max-warnings 0), unit tests, then **Playwright
     e2e against a mocked server**.
   - **apple / windows** — regenerate the API client and `git diff --exit-code` (drift
     check), run the core unit suite, build the app.
3. **`CI passed` aggregate job** (`if: always()`, `needs:` every job) fails if any job
   failed or was cancelled. Set *this* as the required status check so path-skipped jobs
   never leave a required check stuck pending.

**Codegen drift is a first-class gate.** The server emits the OpenAPI contract; every
client is generated from it and CI fails if regeneration produces a diff. This is what
keeps the "contract = `api/openapi.json`" rule honest.

## Runners

- Linux jobs: `runs-on: [self-hosted, Linux, X64]`.
- Apple jobs: `runs-on: [self-hosted, macOS, ARM64]` (the dev Mac — see
  [self-hosted-agents.md](self-hosted-agents.md)).
- Windows: `runs-on: windows-latest` until a self-hosted Windows runner exists.
- **`concurrency: { group: <name>-${{ github.ref }}, cancel-in-progress: true }}`** on every
  workflow, so a newer push supersedes in-flight runs instead of queuing.

### Self-hosted runner gotchas (bake these in)

- **Private crates over SSH** (`liblog`, `libproduct`, `auth-sdk`, the `lib*` family):
  load the deploy key with `webfactory/ssh-agent@v0.9.0`, and set
  `CARGO_NET_GIT_FETCH_WITH_CLI: "true"` + `GIT_SSH_COMMAND: "ssh -o StrictHostKeyChecking=accept-new"`
  (libgit2's ssh-agent auth is unreliable on runners).
- **No passwordless sudo** on the agents: install `protoc` via `arduino/setup-protoc@v3`
  (not apt); pre-install Playwright system libs on the agent once, then CI only does
  `npx playwright install chromium` (no `--with-deps`).
- **Shared filesystem**: give each Rust job its own `CARGO_TARGET_DIR`
  (`target-build` vs `target-coverage`) so build and coverage don't race on `server/target`.

## Delivery (`*.build` / `*.release`)

- **Services → GHCR.** `server.build.yml` / `web.build.yml` build the Docker image and push
  `ghcr.io/<owner>/<product>-{api,web}:main` via `docker/build-push-action@v6`, with
  `push: ${{ github.event_name != 'pull_request' }}` and `cache-{from,to}: type=gha`.
  `RELEASE_VERSION` is passed as a `--build-arg`. Private-crate SSH is forwarded with
  `ssh: default`.
- **Client apps → registry.** `{ios,macos,windows,cli}.release.yml` build signed/notarized
  artifacts and publish to `apps.dev.thmsn.dev`. See [registry-publishing.md](registry-publishing.md).

Templates: [`templates/github-workflows/`](../templates/github-workflows/) (`ci.yml`,
`server.build.yml`, `web.build.yml`).

## Workflow naming

Workflows are named **component-first, dot-namespaced**, so the `.github/workflows/` listing
groups by the surface each one delivers (every `server.*`, every `apple`/`ios.*` together).

| Element | Rule | Example |
|---|---|---|
| **File** | `<component>.<action>[-<qualifier>].yml` | `server.build.yml`, `web.build.yml`, `macos.release.yml`, `cli.release.yml` |
| **`name:`** | `<component> · <action>` | `server · build image`, `macos · release` |
| **Job id** | short verb | `build`, `release`, `coverage` |
| **Job `name:`** (optional) | richer human phrase, `·`-separated | `build · clippy · test` |
| **Step `name:`** | imperative, sentence case (parenthetical qualifier ok) | `Resolve version`, `Clippy (warnings are errors)` |
| **Concurrency group** | `<component>.<action>-${{ github.ref }}` + `cancel-in-progress: true` | `server.build-${{ github.ref }}` |

- **Component = the deliverable surface**: `server`, `web`, `ios`, `macos`, `windows`, `cli`.
  Apple's two targets are separate surfaces (`ios.release.yml`, `macos.release.yml`), not one
  `apple.*` with a hidden split.
- **Action** is the verb: `build` (image → GHCR), `release` (artifact → registry).
- **`ci.yml` is the one exception** — it's deliberately cross-component (the aggregate gate),
  so it keeps the bare name `ci.yml`/`CI` and carries per-component **jobs** inside
  (`server`, `web`, `apple`, `windows`).
- **Workflow surface ≠ identifier component.** A workflow surface names the OS/runner target
  (`windows`), which deliberately differs from the app's identifier component (`winui`) — see
  [identifiers.md](identifiers.md). Use `windows` in workflow names, `winui` in identifiers.

## Checklist

- [ ] `ci.yml` triggers on `push: main` + `pull_request`, with `concurrency` cancel.
- [ ] `paths-filter` gates per-area jobs; OpenAPI changes re-run all clients.
- [ ] Clippy/lint run with warnings-as-errors; tests run; tarpaulin enforces 80%.
- [ ] Every generated client has a drift check (`git diff --exit-code`).
- [ ] One aggregate "CI passed" job is the required status.
- [ ] `*.build` push images only on `main`; `RELEASE_VERSION` computed once and passed in.
- [ ] Workflows named `<component>.<action>.yml` (component-first); `ci.yml` is the lone exception.
- [ ] Runner gotchas handled: SSH agent + git-fetch-with-cli, protoc, per-job target dirs.
