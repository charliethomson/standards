# Registry publishing

## Rule

Client-installable artifacts (iOS/macOS/Windows apps, CLI tools) ship through the
self-hosted registry at **`apps.dev.thmsn.dev`** — not GHCR (that's for service container
images, see [deployment.md](deployment.md)). Publishing is one CI step using the registry's
composite action.

## Two kinds

- **`app`** — a GUI app (`.ipa` / `.dmg` / `.exe` / `.msi`). iOS apps are served through a
  dynamically generated AltStore/SideStore `source.json` (source identifier `dev.thmsn.apps`,
  which must never change); macOS/Windows are direct downloads with version history.
- **`cli`** — a command-line tool with **multi-target artifacts** (one per os/arch/libc)
  under a single version, plus an **install manifest** (see
  [archetypes/cli-tool.md](archetypes/cli-tool.md)). Installed via the registry's
  platform-detecting installer (`GET /install/<slug>`, `…/install/<slug>.ps1`).

Every upload is **SHA-256 verified** by the server.

## Publish from CI

Use the composite action `charliethomson/registry/.github/actions/publish@main` in a
`<surface>.release.yml` workflow (e.g. `macos.release.yml`), after building (and
signing/notarizing) the artifact. Pass the
CI-computed `RELEASE_VERSION` ([versioning.md](versioning.md)) as `version`.

```yaml
- name: Publish to registry
  uses: charliethomson/registry/.github/actions/publish@main
  with:
    registry-url: ${{ vars.REGISTRY_URL }}      # https://apps.dev.thmsn.dev
    token: ${{ secrets.REGISTRY_TOKEN }}        # REGISTRY_AUTH_TOKEN
    slug: <product>
    version: ${{ env.RELEASE_VERSION }}
    file: dist/${{ env.RELEASE_VERSION }}.ipa   # app kind
    platform: ios                                # ios | macos | windows
    min-os: "18.0"
    notes: ${{ env.NOTES }}
    # first-time create (app is created when missing):
    meta: apps/apple/altstore/meta.json          # or individual name/bundle-id/developer fields
    icon: apps/apple/altstore/icon.png
```

For a CLI, set `kind: cli` and pass `artifacts` as `TARGETS=PATH` lines
(`macos-universal=dist/tool-macos.tar.gz`) instead of `file`.

Useful inputs: `build` (CFBundleVersion / `CURRENT_PROJECT_VERSION` = commit count),
`channel` (default `stable`), `skip-existing: "true"` (a duplicate version is a no-op
instead of a failure), `bundle-id` (required for iOS on first create).

### Required repo config

| Kind | Name | Value |
|---|---|---|
| variable | `REGISTRY_URL` | `https://apps.dev.thmsn.dev` |
| secret | `REGISTRY_TOKEN` | the server's `REGISTRY_AUTH_TOKEN` |

A portable script (`scripts/registry-publish.sh` in the registry repo) and a raw
`POST /api/v1/apps/<slug>/versions` are also available for non-Actions contexts.

## Checklist

- [ ] Client artifacts publish to `apps.dev.thmsn.dev`, not GHCR.
- [ ] `*.release.yml` uses `charliethomson/registry/.github/actions/publish@main`.
- [ ] `version` = the CI `RELEASE_VERSION`; `build` = commit count.
- [ ] `REGISTRY_URL` variable + `REGISTRY_TOKEN` secret set on the repo.
- [ ] First publish provides `meta`/`icon` (or the individual create fields); iOS sets `bundle-id`.
- [ ] CLI tools publish `kind: cli` with per-target `artifacts` + an install manifest.
