# Self-hosted agents

## Rule

CI builds run on **self-hosted GitHub Actions runners**, managed as a fleet by
**agentutil** (deployed at `agentutil.dev.thmsn.dev`). A repo
opts a job onto the fleet with `runs-on: [self-hosted, …]`.

## What an "agent" is

A remote machine running one or more GitHub Actions self-hosted runners. agentutil tracks a
desired-state `(repository, agent)` matrix in a database and reconciles it over SSH:
installing/removing runners, pinning host keys (TOFU), and reporting drift (runners on
GitHub with no assignment). Each host is prepared once with `setup-agent.sh`, which
establishes the contract agentutil relies on:

- a `github-runner` service user that runs the runner daemon,
- an install base at `/opt/actions-runner` (subdivided per repo as `<owner>-<repo>`),
- helper scripts (`runner-fixown`, `runner-config-clean`, `runner-reclaim`) with scoped
  passwordless sudo,
- docker group access and nightly disk hygiene.

You then register the host + an SSH key in agentutil and assign repositories to it; the
runner name is `<prefix>-<agent-name>`.

## How a repo uses it

Pick the runner label for the job's platform:

| Job | `runs-on` | Notes |
|---|---|---|
| Linux build/test/coverage, image builds | `[self-hosted, Linux, X64]` | the workhorse |
| Apple build + notarize | `[self-hosted, macOS, ARM64]` | the dev Mac (below) |
| Windows | `windows-latest` | GitHub-hosted until a self-hosted Windows runner exists |
| Browser e2e (Playwright) | GitHub-hosted or Linux self-hosted | mind the no-sudo gotcha |

The runner constraints shape the workflow — see the **self-hosted runner gotchas** in
[ci-cd.md](ci-cd.md) (SSH agent for private crates, `protoc` via setup-protoc, no
`--with-deps`, per-job `CARGO_TARGET_DIR`). These exist because the agents have **no
passwordless sudo** and a **shared filesystem**.

## The dev Mac as the macOS agent

Apple builds (iOS/macOS archive, sign, notarize) need macOS and Apple toolchains, so **the
dev machine acts as the macOS agent** — registered like any other agent, labelled
`[self-hosted, macOS, ARM64]`. It's where `macos.release.yml` / `ios.release.yml` produce
notarized artifacts before publishing to the registry. agentutil's host-prep is Linux-only
today, so the Mac is prepared manually; the runner registration is the same.

## Scope

This is about **CI runners**, not deploy targets. Deployment is Komodo Periphery (a
different kind of agent) — see [deployment.md](deployment.md). agentutil itself is a
full-stack product deployed that way.

## Checklist

- [ ] CI jobs use `runs-on: [self-hosted, …]` with the right platform labels.
- [ ] The host is prepared via `setup-agent.sh` and registered + assigned in agentutil.
- [ ] Apple jobs target the dev Mac (`[self-hosted, macOS, ARM64]`).
- [ ] Workflows account for no-sudo / shared-FS runner constraints (see ci-cd.md).
- [ ] Windows stays on `windows-latest` until a self-hosted Windows runner exists.
