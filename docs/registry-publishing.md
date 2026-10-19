# Registry publishing

## Rule

Client-installable artifacts (iOS/macOS/Windows apps, CLI tools) ship through the
self-hosted registry at **`apps.dev.thmsn.dev`** — not GHCR (that's for service container
images, see [deployment.md](deployment.md)). The registry is **product-oriented**: a
**product** — identified by its reverse-DNS identifier per
[identifiers.md](identifiers.md) (e.g. `dev.thmsn.someproduct`) — owns one or more
**apps** (distribution units addressed by a slug within the product, e.g. `ios`, `macos`,
`winui`, `cli`), and each app is individually versioned.

## Why

One product repo usually ships several surfaces (an iOS build, a macOS build, a CLI) that
share a name, developer, icon, and homepage. Keying distribution on the product identifier
lets those surfaces live under one entity with shared metadata, while each app keeps its own
platform, bundle id, and version history. The reverse-DNS identifier is the same one the repo
already uses everywhere else, so there's no second namespace to invent.

## Two kinds of app

- **`app`** — a GUI app (`.ipa` / `.dmg` / `.exe` / `.msi`). iOS apps are served through a
  dynamically generated AltStore/SideStore `source.json` (source identifier `dev.thmsn.apps`,
  which must never change); macOS/Windows are direct downloads with version history.
- **`cli`** — a command-line tool with **multi-target artifacts** (one per os/arch/libc)
  under a single version, plus an **install manifest** (see
  [archetypes/cli-tool.md](archetypes/cli-tool.md)). Installed via the registry's
  platform-detecting installer at `/install/<product>/<app>` — or the shorter
  `/install/<product>` when the product has exactly one `cli` app.

Every upload is **SHA-256 verified** by the server. The publish flow is always the same:

1. **ensure the product exists** (once — identifier, name, developer, description, icon, …),
2. **ensure the app exists** under it (once — slug, kind, platforms, bundle id, …),
3. **publish a version** — upload the built binary; the server computes and verifies its
   SHA-256 and (for iOS) exposes it in `source.json`.

Writes need the bearer token configured on the server (`REGISTRY_AUTH_TOKEN`); reads are
public.

## Publish from CI

The inputs, the ordering, and the draft → artifacts → finalize path below are the contract
however it is invoked. Which invocation depends on the repo's CI:

| Repo | Publishes with |
|---|---|
| **Woodpecker, on kiln** (`.kiln/`) | `libci::registry::AppRegistry` — below |
| **Woodpecker, pre-kiln** | a copied `scripts/ci/publish.sh`. Don't copy it into another repo; port the release pipeline to kiln instead. If the repo has no publish path yet, keep the release pipeline `event: manual` rather than shipping a trigger it cannot honour. |
| **GitHub Actions** (legacy) | the composite action — [below](#github-actions-legacy) |

Woodpecker has no `uses:` and cannot call the composite action at all ([ci-cd.md](ci-cd.md)).

### kiln: `libci::registry`

[`AppRegistry`](https://github.com/charliethomson/libci/blob/main/src/registry.rs) (feature `registry`) wraps the registry's client
script once for the fleet, replacing every per-repo `publish.sh`. Pass it the `Version` from
[`libci::version`](versioning.md); it sends `--version` = `RELEASE_VERSION` and
`--build` = the commit count.

```rust
use libci::registry::{AppRegistry, release_notes};
use libci::version::Version;

// a GUI app
AppRegistry::declare(j.on(Macos).lock(Signing)).run(|ctx| {
    let v = Version::derive(ctx)?;
    // … build, sign, notarize …
    let registry = AppRegistry::connect(ctx)?;
    registry.ensure_product("dev.thmsn.someproduct").meta("apps/apple/altstore/meta.json").call()?;
    registry.ensure_app("dev.thmsn.someproduct", "macos").platforms(["macos"]).call()?;
    registry
        .publish("dev.thmsn.someproduct", "macos", &v, "dist/SomeProduct.dmg")
        .min_os("26.0")
        .notes(release_notes(ctx, &format!("SomeProduct {v}"))?)
        .skip_existing()
        .call()
})

// a CLI: each per-target leg attaches and leaves a draft …
registry.publish_cli("dev.thmsn.someproduct", "cli", &v)
    .artifact(["linux-amd64", "linux-arm64"], "dist/tool-linux.tar.gz")
    .bin_name("sometool").install("packaging/install.json")
    .no_finalize().skip_existing().call()?;
// … and one fan-in job flips it live
registry.finalize("dev.thmsn.someproduct", "cli", &v)
    .require_targets(["macos-universal", "linux-amd64", "linux-arm64"]).call()?;
```

- **`AppRegistry::declare(job)`** adds both secrets (`registry_token`, `registry_repo_token`)
  and `.requires(Tailnet)` in one call — the registry does not resolve off the tailnet. The
  secrets are never allowed on a pull request.
- **`connect`** clones the registry's client from the private `charliethomson/registry` with
  `registry_repo_token`. **The token travels as a git `http.extraHeader`** set through
  `GIT_CONFIG_COUNT` in that one `git` process's environment — **not in the clone URL**, where
  it showed in `ps` on a shared agent and was written into `.git/config`. The clone never
  prompts (a blank token fails before anything runs) and times out at two minutes.
- **`ensure_product` / `ensure_app`** are idempotent; `.update()` PATCHes existing metadata.
- **`publish` / `publish_cli`** check the artifact exists before uploading. Release notes go
  through the environment, never argv.
- **`finalize` fails unless the registry actually finalized.** `--require-targets` exits 0 on
  an incomplete set; in a fan-in that would leave the version a draft behind a green tick.

### GitHub Actions (legacy)

Use the composite action `charliethomson/registry/.github/actions/publish@main` in a
`<surface>.release.yml` workflow (e.g. `macos.release.yml`), after building (and
signing/notarizing) the artifact. Pass the product identifier as `product`, the app slug as
`app`, and the CI-computed `RELEASE_VERSION` ([versioning.md](versioning.md)) as `version`.
First-time create fields (`platforms`, `meta`/`product-icon`, or the individual metadata
inputs) are idempotent — safe to leave in every run.

```yaml
- name: Publish to registry
  uses: charliethomson/registry/.github/actions/publish@main
  with:
    registry-url: ${{ vars.REGISTRY_URL }}      # https://apps.dev.thmsn.dev
    token: ${{ secrets.REGISTRY_TOKEN }}        # REGISTRY_AUTH_TOKEN
    product: dev.thmsn.someproduct
    app: ios                                     # ios | macos | winui | cli
    version: ${{ env.RELEASE_VERSION }}
    file: dist/${{ env.RELEASE_VERSION }}.ipa    # app kind
    platforms: ios                               # ios | macos | windows (first-time create)
    min-os: "18.0"
    notes: ${{ env.NOTES }}
    # first-time create metadata (idempotent):
    meta: apps/apple/altstore/meta.json          # or individual name/developer/… fields
    product-icon: apps/apple/altstore/icon.png
```

For a CLI, set `kind: cli` and pass `artifacts` as `TARGETS=PATH` lines
(`macos-universal=dist/tool-macos.tar.gz`) instead of `file` — this switches the action to the
draft → artifacts → finalize path so a multi-runner matrix can attach each platform's archive
and flip the whole version live atomically (see [archetypes/cli-tool.md](archetypes/cli-tool.md)).

Required inputs: `registry-url`, `token`, `product`, `app`, `version`, and `file` (or
`artifacts` for `kind: cli`). Version metadata: `min-os`, `build`
(CFBundleVersion / `CURRENT_PROJECT_VERSION` = commit count), `channel` (default `stable`),
`notes`, `skip-existing: "true"` (a duplicate version is a no-op instead of a failure).
Create-time metadata (only applied when the product/app is missing): `meta` **or** the product
fields (`name`, `developer`, `description`, `homepage`, `repo`, `tint`, `category`,
`product-icon`) plus the app fields (`platforms`, `bundle-id`, `app-name`, `subtitle`, `icon`,
`bin-name`, `install`). Set `update: "true"` to PATCH metadata that already exists. For iOS the
bundle id must match the IPA's `CFBundleIdentifier` (defaults to `<product>.<app>` on create).

### Required repo config

| CI | Kind | Name | Value |
|---|---|---|---|
| Actions | variable | `REGISTRY_URL` | `https://apps.dev.thmsn.dev` |
| Actions | secret | `REGISTRY_TOKEN` | the server's `REGISTRY_AUTH_TOKEN` |
| Woodpecker | secret | `registry_token` | the server's `REGISTRY_AUTH_TOKEN` |
| Woodpecker | secret | `registry_repo_token` | a read-only token that can clone `charliethomson/registry` |

## The script directly

A portable script (`scripts/registry-publish.sh` in the registry repo; bash + curl +
`sha256sum`/`shasum`, `jq` only for `--meta`/`--install`) exposes the same three-step flow for
non-Actions contexts. `ensure-product` and `ensure-app` are idempotent (no-op if the entity
exists, unless `--update`); `publish`/`publish-cli` exit non-zero on a duplicate version unless
`--skip-existing`.

```sh
export REGISTRY_URL=https://apps.dev.thmsn.dev REGISTRY_TOKEN=…

# once
scripts/registry-publish.sh ensure-product --product dev.thmsn.someproduct \
  --meta apps/apple/altstore/meta.json --icon apps/apple/altstore/icon.png
scripts/registry-publish.sh ensure-app --product dev.thmsn.someproduct --app ios \
  --platforms ios --meta apps/apple/altstore/meta.json

# each release (apps)
scripts/registry-publish.sh publish --product dev.thmsn.someproduct --app ios \
  --version "$RELEASE_VERSION" --file "dist/$RELEASE_VERSION.ipa" --min-os 18.0 \
  --notes "$(git log -1 --pretty=%B)"

# each release (cli — draft, attach per-target archives, finalize)
scripts/registry-publish.sh publish-cli --product dev.thmsn.someproduct --app cli \
  --version "$RELEASE_VERSION" --bin-name sometool --install packaging/install.json \
  --artifact "macos-universal=dist/tool-macos.tar.gz" \
  --artifact "linux-amd64,linux-arm64=dist/tool-linux.tar.gz"
```

The raw endpoint is `POST /api/v1/products/<product>/apps/<app>/versions` (multipart:
`version`, `sha256`, `file`, optional `min_os_version`/`release_notes`). Status codes: `200`
published · `400` checksum mismatch · `401` bad token · `404` unknown product/app (run
`ensure-product` + `ensure-app` first) · `409` version already exists. The full API is at
`$REGISTRY_URL/docs`.

## Checklist

- [ ] Client artifacts publish to `apps.dev.thmsn.dev`, not GHCR.
- [ ] Release pipelines publish through `libci::registry::AppRegistry` (kiln), or the
      composite action (Actions) — not a newly copied `scripts/ci/publish.sh`.
- [ ] `product` = the repo's reverse-DNS identifier; `app` = the surface slug (`ios`/`macos`/`winui`/`cli`).
- [ ] `version` = the CI `RELEASE_VERSION`; `build` = commit count.
- [ ] Secrets provisioned: `registry_token` + `registry_repo_token` (Woodpecker), or the
      `REGISTRY_URL` variable + `REGISTRY_TOKEN` secret (Actions).
- [ ] First publish provides `platforms` + `meta`/`product-icon` (or the individual create fields); iOS sets `bundle-id`.
- [ ] CLI tools publish `kind: cli` with per-target `artifacts` + an install manifest.
