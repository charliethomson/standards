---
name: thmsn-standards
description: Audit the current repo against the shared engineering standards (the standards/ submodule) and report — or fix — the findings. Use when asked to check/audit/enforce "the standards", verify a repo follows them, or fix standards violations. Example: "/thmsn-standards scan this repo and fix all findings".
---

# thmsn-standards — audit & fix

Audit this repo against the standards vendored at `standards/`, then report findings and
(if asked) fix them. The standards docs are the source of truth — **drive the audit from each
applicable doc's `## Checklist` section**, not from memory, so this stays correct as the
standards evolve.

## 1. Establish scope

- Read `.standards.conf` at the repo root → `ARCHETYPE`, `PRODUCT`, `PRODUCT_UPPER`, `LIB`.
  If it's missing, the repo isn't wired up: tell the user to run
  `standards/bin/standards install --product <slug> --archetype <type>` first, and offer to
  infer the archetype (`full-stack-product` / `library` / `orchestration` / `cli-tool`).
- Read `standards/docs/archetypes/<ARCHETYPE>.md` → its **"Standards that apply"** table.
  That table is the authoritative list of which standards to audit and how hard each applies
  (some are marked N/A for this archetype — skip those).

## 2. Load the rules

For each applicable standard, read `standards/docs/<standard>.md` — focus on its `## Checklist`
(the concrete, verifiable items) and skim the body for the intended implementation. Also apply
the archetype doc's own checklist.

## 3. Audit

Check each checklist item against the repo and assign a status with **evidence** (`file:line`):

- ✅ **pass** — satisfied; cite where.
- ❌ **fail** — violated or missing; cite the gap.
- ⚠️ **partial** — present but not to standard.
- ➖ **n/a** — genuinely doesn't apply here (say why).

Heuristics for the items that need more than a glance:

- **Versioning** — is there a `build.rs` deriving `MAJOR.MINOR.<count>`? Does CI compute
  `RELEASE_VERSION` (checkout `fetch-depth: 0` + `fetch-tags: true`)? Are manifest versions
  static placeholders (not hand-bumped)? Is there a root `VERSIONING.md`?
- **Identifiers** — grep for the product root `dev.thmsn.<PRODUCT>`; binaries set it via
  `product_name!`; components named `dev.thmsn.<PRODUCT>.<component>`; reverse-domain only.
- **CI/CD** — workflows named component-first (`server.build.yml`, `*.release.yml`), `ci.yml`
  is the gate; self-hosted runners; coverage + codegen-drift gates present.
- **Deployment** — `deploy/` has compose + internal Caddyfile + komodo sync; no secrets in git.
- **Configuration** — config via `libconfig` (`Loader`/`config!{}`), not hand-rolled `std::env::var`.
- **Testing** — services have `tarpaulin.toml` with `fail-under = 80`; UIs are not unit-tested.
- **Observability / Error handling / Service architecture / etc.** — verify against their checklists.

Mark anything you can't determine confidently as **needs review** rather than guessing.

## 4. Report

Output a findings report grouped by standard, each item one line:

```
## <standard>
  ❌ <checklist item> — <evidence/gap> → <fix>
  ✅ <checklist item> — <where>
```

End with a summary count and a prioritized fix list (mechanical fixes first, judgment calls last).

## 5. Fix (only when asked)

If the user wants findings fixed, apply them in two tiers:

- **Auto-fix (mechanical, low-risk)** — do these directly:
  copy a missing template from `standards/templates/` into place and fill placeholders
  (`{{PRODUCT}}`→`$PRODUCT`, `{{PRODUCT_UPPER}}`→`$PRODUCT_UPPER`); add a missing
  `VERSIONING.md`/`tarpaulin.toml`/`config.rs`/workflow; rename workflows to the component-first
  scheme; fix an identifier string; add a `.cargo/config.toml` flag. Re-verify after each.
- **Confirm first (judgment / large / destructive)** — propose a concrete diff and wait:
  crate restructuring, adding test coverage to hit 80%, rewriting deploy config, anything that
  deletes or moves significant code, or anything where the standard offers a choice.

Never hand-edit a `*.generated.*` file or the generated root `AGENTS.md` (beyond its
"repo-specific overrides" section). Pull templates from `standards/templates/`; don't reinvent.

After fixing, re-run the audit on the touched areas, summarize what changed vs. what still needs
the user, and leave the changes staged (don't commit unless asked).
