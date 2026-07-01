# Archetype: Vendored app

A third-party application **we do not own**, pinned as an `upstream/` submodule and wired
into our own build/release infrastructure. We don't fork it, rebrand it, or reimplement it —
we compile what upstream shipped, apply only the mechanical adjustments our signing/registry
setup needs, and publish it to `apps.dev.thmsn.dev` like any other fleet app.

This is the **à-la-carte** archetype: the standards are a reference menu here, not a binding
set. Only the release-pipeline standards apply; everything about how the app is *built*
internally (its architecture, data, contracts, auth, branding, platform UX) belongs to
upstream and is **N/A**.

Reference: `relay` — packaging [subpop/Relay](https://github.com/subpop/Relay), a native
macOS Matrix client, into a signed, notarized `.dmg` in the registry.

## When this — not full-stack-product

Reach for this archetype when **all** of these hold; otherwise it's a real product and one
of the other archetypes applies:

- The source of truth for the app lives in someone else's repo. We track it, we don't author it.
- We add no features and reimplement no behaviour — changes are limited to what's needed to
  *build and sign* it (materialising a config file, stripping a capability we can't provision).
- We keep upstream's identity. **No rebrand** — bundle ids, product names, and icons stay theirs.

If you find yourself editing upstream source to change behaviour, you've forked it, not
vendored it — that's a product, not this archetype.

## Repo shape

```
<product>/
├── upstream/                 the vendored app, a PINNED third-party submodule (never edited)
├── scripts/                  build-time adjustments applied to a checkout, never committed upstream
│   ├── strip-*.sh            remove a capability we can't/won't provision (self-verifying)
│   └── extract-app-icon.sh   render the registry icon from the built bundle
├── .github/workflows/        <platform>.release.yml → build, sign, notarize, publish
├── .standards.conf           PRODUCT = tooling slug only (NOT applied to the vendored app)
├── AGENTS.md                 stub → standards/, with the à-la-carte overrides spelled out
└── standards/                this submodule
```

The distinguishing features: `upstream/` is a **pinned** submodule (a release is a
deliberate bump of that pointer, not a push to this repo), and all divergence from upstream
lives in `scripts/` applied at **build time** — the upstream checkout is never mutated in git.

## Standards that apply

| Standard | How it applies here |
|---|---|
| [CI/CD](../ci-cd.md) | **The defining standard** — `<platform>.release.yml` builds/signs/notarizes on a self-hosted runner and publishes. Trigger is a bump of the `upstream/` submodule pointer (or manual dispatch). |
| [Self-hosted agents](../self-hosted-agents.md) | The signing identity + registry access live on the self-hosted runner (e.g. the dev Mac for a notarized macOS build). |
| [Registry publishing](../registry-publishing.md) | **Also defining** — published to `apps.dev.thmsn.dev`, attributed to upstream (`developer`, `category: external`). |
| [Versioning](../versioning.md) | **Adapted** — ship *upstream's own* version (read from its git tag), **not** our derived `MAJOR.MINOR.<count>`. We release what upstream tagged. |
| [Build info](../build-info.md) | As needed to feed the release metadata (upstream commit/tag → registry version + build number). We don't inject `libbuildinfo` into code we don't own. |
| [Workflow](../workflow.md) | Commit to `main`; a release is a submodule-pointer bump on `main`. |
| [Identifiers](../identifiers.md) | **Tooling only.** `dev.thmsn.<product>` is the standards/registry slug for *this repo*; it is **never** applied to the vendored app, which keeps its upstream bundle ids. **No rebrand.** |
| Service architecture / Data / Public ids / Contracts / Error handling / Auth / Configuration / Observability / Security / Web & client arch / Rust conventions / `lib*` ecosystem | **N/A** — upstream owns how the app is built. |
| Branding / Platform UX | **N/A** — we ship upstream's branding and UX unchanged. |
| Testing | **N/A** — we don't test code we don't own; the release workflow's build/sign/notarize/staple steps are the gate. |

## No rebrand, build-time only

Two rules keep this honest and keep upstream re-syncs cheap:

- **No rebrand.** The published artifact keeps upstream's identity (bundle id, product name,
  icons). The `dev.thmsn.<product>` identifier is only this repo's tooling/registry slug — it
  never touches the vendored code. In the registry the app is attributed to upstream and
  flagged `category: external`.
- **Build-time adjustments only.** Any divergence from upstream (stripping a
  provisioning-requiring capability, materialising a secrets config, extracting an icon)
  happens in `scripts/` run *during the build*, against a fresh checkout — never committed
  into the `upstream/` submodule. Make each script **self-verifying** so it fails loudly when
  upstream's layout changes, rather than silently producing a broken build. A version bump is
  then just `git submodule update --remote upstream` + a re-run.

## Checklist

- [ ] `upstream/` is a pinned third-party submodule; a release is a deliberate bump of its pointer.
- [ ] Nothing in `upstream/` is edited in git — all divergence lives in `scripts/`, applied at build time.
- [ ] Build-time scripts are self-verifying (fail loudly if upstream's layout changed).
- [ ] Release workflow builds/signs/notarizes on a self-hosted runner and publishes to the registry.
- [ ] Published version is **upstream's** (from its tag), not the derived scheme; attributed to upstream, `category: external`.
- [ ] No rebrand — the artifact keeps upstream's bundle id / name / icons; `dev.thmsn.<product>` is tooling-only.
- [ ] `AGENTS.md` spells out the à-la-carte overrides (which standards apply, what's N/A, and that there's no rebrand).
