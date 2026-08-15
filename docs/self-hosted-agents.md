# Self-hosted agents

## Rule

CI builds run on **self-hosted agents**, on hardware we own. There are **two substrates**,
because the fleet is mid-migration ([ci-cd.md](ci-cd.md)):

- **Woodpecker agents — the standard.** The Woodpecker server runs in the homelab; a repo
  opts a pipeline onto an agent with `labels:`. Linux agents run the **docker** backend and
  are deployed by Komodo alongside the server; the **macOS and Windows agents run the `local`
  backend, are hand-managed, and have no checked-in configuration**.
- **GitHub Actions runners — legacy**, for repos not yet converted. These are the fleet
  **agentutil** (at `agentutil.dev.thmsn.dev`) manages, and a job opts in with
  `runs-on: [self-hosted, …]`.

**agentutil manages only the Actions runners.** It has not been extended to Woodpecker, and
Woodpecker agents are not visible to it. Do not expect one tool to show you the whole fleet.

## What an Actions "agent" is

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

## How a repo picks an agent

| Job | Woodpecker `labels:` | legacy Actions `runs-on` |
|---|---|---|
| Linux build/test/coverage, image builds | `platform: linux/amd64` | `[self-hosted, Linux, X64]` |
| Apple build + notarize | `os: macos` | `[self-hosted, macOS, ARM64]` |
| Windows build/release | `os: windows` | `windows-latest` |
| Browser e2e (Playwright) | `platform: linux/amd64` | GitHub-hosted or Linux self-hosted |

**A self-hosted Windows agent now exists** (hand-managed, on the tailnet, labelled
`os: windows`), so Windows work no longer has to fall back to a GitHub-hosted runner.

The agent constraints shape the pipeline — see [ci-cd.md](ci-cd.md) (SSH agent for private
crates, per-step `CARGO_TARGET_DIR`, bounded caches, the local-backend rules). Note the
constraints **differ by substrate**: the Actions runners have **no passwordless sudo** and a
**shared filesystem**, whereas a Woodpecker Docker step runs as root in a fresh container —
so the sudo workarounds are unnecessary there, and the caching ones are mandatory.

## The dev Mac as the macOS agent

Apple builds (iOS/macOS archive, sign, notarize) need macOS and Apple toolchains, so **the
dev machine acts as the macOS agent**. It's where `macos.release.yml` / `ios.release.yml`
produce notarized artifacts before publishing to the registry. Host prep is Linux-only in
both substrates, so the Mac is prepared manually either way.

It is also where anything requiring **byte-exact output** must run. Rasterised PNGs are not
reproducible across librsvg/cairo builds, so a branding drift check that diffs generated
icons has to run on the machine the committed ones were generated on — otherwise it fails on
rasteriser noise, which is a red check nobody can act on. The alternative is to scope the
diff to the deterministic text outputs and drop the image comparison.

## Scope

This is about **CI runners**, not deploy targets. Deployment is Komodo Periphery (a
different kind of agent) — see [deployment.md](deployment.md). agentutil itself is a
full-stack product deployed that way.

## Checklist

- [ ] Every Woodpecker pipeline sets `labels:` — an unlabelled one can land on a local-backend
      agent, and a mismatched one never schedules **silently**.
- [ ] Apple and byte-exact jobs target the dev Mac (`os: macos`); Windows targets `os: windows`.
- [ ] macOS/Windows pipelines handle the local backend: `skip_clone` + hand clone + env script.
- [ ] Legacy Actions repos only: host prepared via `setup-agent.sh`, registered + assigned in
      agentutil, and workflows account for no-sudo / shared-FS constraints.
