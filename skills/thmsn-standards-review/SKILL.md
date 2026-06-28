---
name: thmsn-standards-review
description: Review the current diff/branch against the shared standards — flag only what your CHANGES violate, not pre-existing repo state. Use as a pre-commit / pre-push / PR check, e.g. "review my changes against the standards" or "/thmsn-standards-review".
---

# thmsn-standards-review — diff-scoped standards check

A fast, focused check of *what you're about to land* — unlike `/thmsn-standards`, which audits
the whole repo. Flag only issues introduced or touched by the diff.

## 1. Get the diff

Pick the right scope:
- staged + unstaged: `git diff HEAD`
- a branch vs its base: `git diff origin/main...HEAD` (or the base the user names)

List the changed files; if there are none, say so and stop.

## 2. Map changed files → standards

Only load the standards a change actually touches:

| Changed path | Standards to check |
|---|---|
| `.github/workflows/**` | ci-cd (naming, gates) |
| `server/**`, `*.rs` | service-architecture, error-handling, configuration, observability, rust-conventions, data-persistence |
| `apps/**` (clients) | platform-ux, web-architecture, contracts |
| `api/openapi.json`, generated clients | contracts (drift) |
| `deploy/**` | deployment |
| `Cargo.toml`, manifests, version refs | versioning, rust-conventions |
| identifiers (`dev.thmsn.*`, `product_name!`, bundle ids) | identifiers |
| `branding/**` | branding |
| test files, `tarpaulin.toml` | testing |

Read each relevant `standards/docs/<standard>.md` **Checklist**.

## 3. Review

For each touched standard, judge the **diff** (not the surrounding file) against the checklist.
Report concisely, worst-first:

```
❌ <file:line> — <what the change violates> → <fix> (see standards/docs/<x>.md)
⚠️ <file:line> — <smell / partial>
```

Call out new identifiers that aren't `dev.thmsn.<product>.<component>`, workflows not following
component-first naming, hand-rolled `std::env::var` instead of `libconfig`, logging of bodies/
query strings, missing codegen-drift or coverage on a changed surface, etc.

If the diff is clean, say so plainly.

## 4. Offer, don't impose

Default to review-only. Offer to apply the mechanical fixes (and do them on request); leave
judgment calls to the user. Don't commit.
