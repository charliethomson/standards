# Prompt: create a vision project from a codebase

Hand this to an agent sitting in a repo that has no vision project yet. It infers the project
from what is actually in the tree and creates it.

This is a **one-off bootstrap**. For planning a body of work — deciding scope, settling the
open questions, decomposing it into tasks with blocking relations — use `/thmsn-jarvis`
instead; it produces a backlog an implementer can pick up, which this does not.

---

You are creating a vision project for the repository you are sitting in, with the `vision` CLI
(see the `vision` skill for the full flag reference).

## 1. Read the repo before inferring anything

- `README.md` — the stated purpose, in the author's words. Prefer it over your own summary.
- The manifest (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`) — name and deps.
- `.standards.conf` if present — `PRODUCT` is the canonical slug and `ARCHETYPE` tells you what
  kind of thing this is. Use them rather than guessing a name.
- Directory shape, `docs/`, and the CI workflows — what is actually built and shipped.
- `git remote get-url origin` — the `owner/name` the project records as its `repo`.

Then settle:

- **Title** — what a person would call it, not the package slug. "User authentication service",
  not `auth-svc`.
- **Slug** — 2–8 uppercase characters, derived from `PRODUCT` (`auth` → `AUTH`). It prefixes
  every ref in the project forever (`AUTH-12`), so short and obvious beats clever.
- **Description** — one to three sentences on what it does. No aspiration, no roadmap.
- **Repo** — `owner/name` from the remote. It is how agents find the project from a checkout.

## 2. Check it does not already exist

```bash
vision projects list
```

There is one project per repo. Match on `repo` first, then on a title or slug that's close.
If something close already exists, or your slug is taken, stop and ask. A duplicate project is
worse than none: work gets split across both and neither shows the real state.

## 3. Create it

```bash
vision projects create \
  --slug <SLUG> \
  --title "<title>" \
  --description "<description>" \
  --repo <owner/name>
```

## 4. Report

Give the project URL (`https://vision.dev.thmsn.dev/p/<SLUG>`), and say what you inferred and
from where — so a wrong inference is obvious and cheap to correct.

## Do not invent a backlog

The temptation is to follow up with placeholder issues: "Project setup", "Core implementation",
"Testing and documentation". Don't. They are indistinguishable from an empty backlog, they
carry no acceptance criteria, and they teach whoever reads the project that its issues are
noise. If real work is known, `/thmsn-jarvis` decomposes it properly; if it is not known, an
empty project is the honest state.
