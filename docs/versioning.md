# Versioning

## Rule

A build's version is **derived from git, never stored in a manifest**:

```
VERSION = <tag MAJOR>.<tag MINOR>.<total commit count>
```

- **`MAJOR.MINOR`** comes from the most recent tag matching `v[0-9]*` (e.g. `v0.2.0` →
  `0.2`). The tag's patch component is ignored. **This is the only manual knob.**
- **The patch component is `git rev-list --count HEAD`** — the total commit count on the
  branch. It is globally monotonic and never resets, so a higher version always means a
  later commit.
- If there is no tag: `0.0.<count>`. If there is no git at all: `0.0.0`.

Manifest versions (`Cargo.toml` `version`, `package.json` `version`) are **static
placeholders** (e.g. `0.0.1`) and are *not* the source of truth. Do not bump them per
release; do not wire release tooling to them.

## Why

One developer, one user, continuous deploy to `main`. Versions exist so that when
something misbehaves you can answer *"which commit is this running?"* instantly — from a
client's About screen, a log line, or a `/version` endpoint. A monotonic commit count
does that with zero ceremony. Marketing-style semver releases would be pure overhead.

Bump `MAJOR.MINOR` by tagging only when you want a human-meaningful boundary (a real
breaking change, a milestone). Everything else rides the commit count.

## Compute it once, in CI

CI is the source of truth. Compute the version once per run and pass it everywhere.

Checkout must fetch full history and tags:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
    fetch-tags: true
```

Then derive `RELEASE_VERSION`:

```sh
COUNT="$(git rev-list --count HEAD)"
TAG="$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)"
MM="$(printf '%s' "${TAG#v}" | awk -F. '{ if ($1 != "" && $2 != "") print $1"."$2 }')"
[ -n "$MM" ] || MM="0.0"
echo "RELEASE_VERSION=$MM.$COUNT" >> "$GITHUB_ENV"
```

## Consume it per platform

| Target | How it receives the version |
|---|---|
| **Rust binary** | `build.rs` reads `RELEASE_VERSION` (or derives from git for local builds) and emits `cargo:rustc-env=APP_VERSION=…`. Wire into clap with `#[command(version = env!("APP_VERSION"))]`. |
| **Docker image** | Pass `--build-arg RELEASE_VERSION=$RELEASE_VERSION`; Dockerfile does `ARG RELEASE_VERSION` → `ENV RELEASE_VERSION=$RELEASE_VERSION` so the inner `build.rs` picks it up. |
| **Apple (Xcode)** | At archive time set `MARKETING_VERSION = $MAJOR.$MINOR.$COUNT` and `CURRENT_PROJECT_VERSION = $COUNT`. |
| **Web** | Inject at build time (e.g. Vite `define` / env) and render it in the UI. |

The canonical `build.rs` is in [`templates/rust/build.rs`](../templates/rust/build.rs).

## Expose it to clients

The point is debuggability, so **surface the version where you'll see it**:

- Log it once on service startup (`info!` with the version string).
- Show it in every client's About / Settings screen.
- Where feasible, embed full build metadata (commit hash, dirty flag, build host, build
  time) via `libbuildinfo` and expose a `/version` or `/health` endpoint or an About
  panel that shows it. See [build-info.md](build-info.md).

A version string alone answers "which commit?"; `libbuildinfo` answers "built from what,
where, when, and was the tree clean?". Use the version string always; add `libbuildinfo`
when the extra detail is worth the dependency.

## Libraries (the exception)

Libraries are consumed over git by commit, so by default they need no tags or per-release
versions: keep a single **workspace-unified** placeholder version in the root `Cargo.toml`
and leave it; consumers pin by git ref. `libbuildinfo` is optional and feature-gated.

The exception: a crate with a **real external consumer** — a published SDK, or a crate
another repo pins to a specific version rather than a git ref — keeps **manually-managed
semver** in `[package].version`. Derivation is for things shipped off `main`; semver is for
contracts. Today the fleet is derive-only; this only kicks in if something like `auth/sdk`
is ever published. See [archetypes/library.md](archetypes/library.md).

## Checklist

- [ ] Manifest versions left as static placeholders (not bumped per release).
- [ ] CI checks out with `fetch-depth: 0` and `fetch-tags: true`.
- [ ] CI computes `RELEASE_VERSION` with the snippet above and passes it to every build.
- [ ] Each artifact exposes its version (startup log, About screen, `/version`).
- [ ] A repo-root `VERSIONING.md` documents this (copy from
      [`templates/VERSIONING.md`](../templates/VERSIONING.md)).
- [ ] `MAJOR.MINOR` bumped only by tagging `vMAJOR.MINOR` at meaningful boundaries.
