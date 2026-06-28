---
name: thmsn-standards-sync
description: Pull the latest shared standards into this repo, summarize what changed upstream, and re-audit what's newly applicable. Use when updating the standards submodule, "sync the standards", or checking whether recent standards changes affect this repo.
---

# thmsn-standards-sync — update & re-audit

Pull upstream standards, explain what changed, and surface anything this repo now needs to do.

## 1. Sync

Run:

```sh
standards/bin/standards sync
```

It updates the `standards/` submodule, prints the upstream commit log between the old and new
pins, re-links any new shared skills into `.claude/skills/`, and stages the pointer bump. If it
reports "already up to date", say so and stop.

## 2. Understand what changed

Read the printed commit log and `standards/CHANGELOG.md` for the new range. Summarize the
changes in plain terms — which standards (docs/templates) were added or amended.

## 3. Map to this repo

Read `.standards.conf` → `ARCHETYPE`, and the archetype's **"Standards that apply"** table. For
each changed standard that applies here, check whether this repo now **diverges** — e.g. a
renamed convention, a new required file, a changed template. Delegate the actual checking to the
`thmsn-standards` audit, scoped to just the changed standards.

## 4. Report

Tell the user:
- what changed upstream (one line each),
- what (if anything) is now newly out of compliance in this repo,
- the suggested fixes (offer to apply the mechanical ones via `/thmsn-standards`).

Then remind them to commit the pointer bump:

```sh
git commit -m "chore: sync standards"
```

(The bump is already staged by `standards sync`.) Don't commit on their behalf unless asked.
