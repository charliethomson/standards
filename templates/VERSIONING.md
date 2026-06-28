# Versioning

Fleet-wide doctrine. The same rules apply to every repo.

## The rule

**Versions are derived, never stored or hand-bumped.**

```
VERSION = <tag MAJOR>.<tag MINOR>.<total commit count>
```

- `MAJOR.MINOR` comes from the most recent `vX.Y[.Z]` git tag — the **only**
  manual knob. Bumping a minor/major = pushing a new tag (`git tag v0.2.0`).
- The third component is the **total commit count** (`git rev-list --count HEAD`),
  so it's globally monotonic — it keeps climbing across minor bumps and never
  resets. This is what the registry's `semver + build` ordering needs.
- Untagged repos fall back to `0.0.<count>`; a build with no git at all falls
  back to `0.0.0`.

Manifest version fields (`Cargo.toml`, `package.json`, …) are **not** the source
of truth. Leave them at a static placeholder; nothing reads them for release.

## Why

The version used to be computed independently in the manifest, in CI, in Xcode,
and in the binary — guaranteed to drift. Deriving one value from git and reading
it everywhere removes the duplication (and the drift) entirely, with zero
per-commit bump commits or hooks. This replaces the old `../version` bump tool.

## Tag convention

- Release-version tags: `vMAJOR.MINOR.PATCH` (e.g. `v0.1.0`). Only `MAJOR.MINOR`
  is read; `PATCH` is ignored (the count supplies the third component).
- The deriver matches `v[0-9]*` only, so platform/app-specific tags
  (e.g. `macos-v0.1.318`) are ignored.
- To bump the marketing version: `git tag v0.2.0 && git push --tags`.

## Mechanism

CI is the source of truth for **released** builds: it computes the version once
and passes it via `RELEASE_VERSION`. Local/dev builds derive the same value from
git, so `--version` is truthful everywhere.

### CI — compute once (shell)

```sh
COUNT="$(git rev-list --count HEAD)"
TAG="$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)"
MM="$(printf '%s' "${TAG#v}" | awk -F. '{ if ($1 != "" && $2 != "") print $1"."$2 }')"
[ -n "$MM" ] || MM="0.0"
RELEASE_VERSION="$MM.$COUNT"
```

Checkout must fetch tags: `actions/checkout@v4` with `fetch-depth: 0` **and**
`fetch-tags: true`.

Use `RELEASE_VERSION` for:
- the registry publish `version`;
- the binary build env (native) or a Docker build-arg (containerized service);
- Xcode `MARKETING_VERSION` (stamped at archive time, not committed). The Xcode
  build number `CURRENT_PROJECT_VERSION` = the commit count.

### Rust binaries — bake it in (`build.rs`)

```rust
// reads RELEASE_VERSION if set (CI), else derives from git, else "0.0.0".
// emits cargo:rustc-env=APP_VERSION  →  #[command(version = env!("APP_VERSION"))]
```

See the someproduct `bin/someproduct/build.rs` for the canonical implementation. Wire clap with
`#[command(version = env!("APP_VERSION"))]` (or expose it on a `/health`/version
endpoint for services).

### Containerized services

`build.rs` can't read `.git` inside a Docker build (the context usually omits it),
so pass the version in from CI as a build-arg and into the process env:

```
docker build --build-arg RELEASE_VERSION="$RELEASE_VERSION" ...
```

and in the Dockerfile, `ARG RELEASE_VERSION` + `ENV RELEASE_VERSION=$RELEASE_VERSION`
so `build.rs` (or a runtime read) picks it up.

## Libraries (the exception)

Crates with a real external consumer (a published SDK, a crate another repo
pins) keep **manually-managed semver** in their `[package].version` — derivation
is for things shipped off `main`, semver is for contracts. Today the whole fleet
is derive-only; revisit if `auth/sdk` (or similar) is ever published.
