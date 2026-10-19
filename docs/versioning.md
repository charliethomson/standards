# Versioning

## Rule

A build's version is **derived from git, never stored in a manifest**:

```
VERSION = <tag MAJOR>.<tag MINOR>.<total commit count>
```

- **`MAJOR.MINOR`** comes from a tag matching `vMAJOR.MINOR[.PATCH]`, anchored at the start
  (e.g. `v0.2.0` → `0.2`; `macos-v0.1.309` is not a release tag). The tag's patch component is
  ignored. **This is the only manual knob.** The tag is chosen in this order:
  1. the nearest one reachable from `HEAD` — `git describe --tags --abbrev=0 --match 'v[0-9]*'`;
  2. failing that, the **highest** `vN.N` tag in the repo, compared **numerically** field by
     field (`0.10` > `0.9`).
- **The patch component is `git rev-list --count HEAD`** — the total commit count on the
  branch. It is globally monotonic and never resets, so a higher version always means a
  later commit.
- **No tag is an error in CI**, not `0.0.<count>`. In a repo that has tags it means the tag
  fetch failed, and publishing anyway ships a version below the live one. A repo that has
  genuinely never been tagged opts in to `0.0.<count>` explicitly, and drops the opt-in when
  its first `v0.1` tag lands.
- **A shallow clone is an error in CI.** `rev-list --count` does not fail on one; it returns a
  smaller number, which is the same below-the-live-version bug.
- Alongside the version, CI exposes **`COMMIT`** (seven-character sha) and **`DIRTY`** (tracked
  files differ from `HEAD`; untracked files do not count).
- Local builds are lenient on purpose: see [build.rs and local builds](#buildrs-and-local-builds).

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

The clone must fetch **full history and tags** — a shallow clone gives the derivation neither
([ci-cd.md](ci-cd.md)):

```yaml
clone:
  git:
    image: woodpeckerci/plugin-git
    settings:
      partial: false
      tags: true
```

**On a kiln repo, the implementation is [`libci::version`](https://github.com/charliethomson/libci/blob/main/src/version.rs)**
(feature `version`): `Version::derive(ctx)` applies every rule above — refuses a shallow
checkout, fetches tags, prefers `describe`, falls back to the highest tag, errors on none — and
returns `major`, `minor`, `build`, `commit` and `dirty`; `release_version()` is the
`RELEASE_VERSION` string. `Version::derive_untagged_ok(ctx)` is the never-tagged opt-in
(`ImageBuild::untagged_ok()` on the image template). Do not reimplement it in a `.kiln/`.

**Pre-kiln repos** keep the derivation in **`scripts/ci/version.sh`** and `eval` it from each
pipeline that needs it, rather than inlining a copy per pipeline. It must implement the same
rule, including the two refusals; the older five-line form that fell back to `0.0` does not:

```sh
set -eu
if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
  echo "shallow clone: the commit count would be too low" >&2; exit 1
fi
git fetch --tags --quiet origin 2>/dev/null || true
mm() { sed -n 's/^v\([0-9][0-9]*\)\.\([0-9][0-9]*\).*/\1 \2/p'; }
MM="$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null | mm)"
[ -n "$MM" ] || MM="$(git tag -l | mm | sort -k1,1n -k2,2n | tail -n 1)"
if [ -z "$MM" ]; then
  [ "${VERSION_UNTAGGED_OK:-}" = 1 ] || { echo "no vMAJOR.MINOR tag (tag fetch failed?)" >&2; exit 1; }
  MM="0 0"
fi
set -- $MM
printf 'RELEASE_VERSION=%s.%s.%s\n' "$1" "$2" "$(git rev-list --count HEAD)"
printf 'COMMIT=%s\n' "$(git rev-parse --short=7 HEAD)"
[ -z "$(git status --porcelain --untracked-files=no)" ] && echo DIRTY=false || echo DIRTY=true
```

Why each piece is there:

- **`describe` first.** It walks only `HEAD`'s ancestry, so a newer tag on a side branch does
  not override the one `main` descends from — and it is the question `build.rs` asks, so the
  two agree in the common case.
- **The highest-tag fallback** covers a `describe` that finds nothing where tags do exist.
- **Numeric sort, field by field.** A lexical sort puts `0.9` after `0.10`, so the first
  `v0.10.x` tag drags every build back below what is already published — which clients read as
  "no update available", permanently. BSD `sort` has no `-V`, and the macOS agent is a real
  target, so use explicit numeric keys.

## Consume it per platform

| Target | How it receives the version |
|---|---|
| **Rust binary** | `build.rs` reads `RELEASE_VERSION` (or derives from git for local builds) and emits `cargo:rustc-env=APP_VERSION=…`. Wire into clap with `#[command(version = env!("APP_VERSION"))]`. |
| **Docker image** | Pass `--build-arg RELEASE_VERSION=$RELEASE_VERSION`; Dockerfile does `ARG RELEASE_VERSION` → `ENV RELEASE_VERSION=$RELEASE_VERSION` so the inner `build.rs` picks it up. |
| **Apple (Xcode)** | At archive time set `MARKETING_VERSION = $MAJOR.$MINOR.$COUNT` and `CURRENT_PROJECT_VERSION = $COUNT`. |
| **Web** | Inject at build time (e.g. Vite `define` / env) and render it in the UI. |

The canonical `build.rs` is in [`templates/rust/build.rs`](../templates/rust/build.rs).

### `build.rs` and local builds

`build.rs` takes `RELEASE_VERSION` when it is set and only derives the version itself when it is
not. That derivation is deliberately **not** the CI rule: it asks `describe` only, has no
highest-tag fallback, never refuses a shallow clone, and falls back to `0.0.<count>` with no
tag and `0.0.0` with no git — because a developer's `cargo build` must not fail for want of a tag.

The two therefore **disagree exactly where CI is strict**: a tag not reachable from `HEAD`
(CI uses the highest tag, `build.rs` says `0.0`), a shallow clone (CI refuses, `build.rs`
undercounts), and no tag at all (CI refuses, `build.rs` says `0.0`). If a CI-built binary ever
derives its own version, an image tagged `0.10.307` can contain a binary reporting `0.0.290`.
So the rule is: **every binary built in CI receives `RELEASE_VERSION`** — as an env var on the
`cargo` step, or as the Docker build-arg plus `ARG`/`ENV` in the Dockerfile (kiln's `GhcrImage`
always passes the build-arg; the Dockerfile still has to declare it). `build.rs`'s own
derivation is for local builds only.

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
- [ ] CI checks out full history and tags — never a shallow clone.
- [ ] CI computes the version with `libci::version::Version::derive` (pre-kiln: the
      `scripts/ci/version.sh` above), and no tag is an error unless the repo has never been tagged.
- [ ] Every CI-built binary and image receives `RELEASE_VERSION`; none relies on `build.rs` deriving it.
- [ ] Each artifact exposes its version (startup log, About screen, `/version`).
- [ ] A repo-root `VERSIONING.md` documents this (copy from
      [`templates/VERSIONING.md`](../templates/VERSIONING.md)).
- [ ] `MAJOR.MINOR` bumped only by tagging `vMAJOR.MINOR` at meaningful boundaries.
