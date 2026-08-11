# Prompt: create a Linear project from a codebase

Hand this to an agent sitting in a repo that has no Linear project yet. It infers the project
from what is actually in the tree and creates it.

This is a **one-off bootstrap**. For planning a body of work — deciding scope, settling the
open questions, decomposing it into tasks with blocking relations — use `/thmsn-jarvis`
instead; it produces a backlog an implementer can pick up, which this does not.

---

You are creating a Linear project for the repository you are sitting in.

## 1. Read the repo before inferring anything

- `README.md` — the stated purpose, in the author's words. Prefer it over your own summary.
- The manifest (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`) — name and deps.
- `.standards.conf` if present — `PRODUCT` is the canonical slug and `ARCHETYPE` tells you what
  kind of thing this is. Use them rather than guessing a name.
- Directory shape, `docs/`, and `.github/workflows/` — what is actually built and shipped.

Then settle:

- **Name** — what a person would call it, not the package slug. "User authentication service",
  not `auth-svc`.
- **Description** — one to three sentences on what it does. No aspiration, no roadmap.
- **Status** — `planned` if nothing is built yet, `started` if there is working code. Run
  `linear projects statuses` first; a workspace only has the statuses it has configured.
- **Icon** — an emoji if the theme is obvious (🔐 auth, 🚀 deploy, 📊 analytics). Skip it
  otherwise rather than picking something arbitrary.

## 2. Pick the team

```bash
linear teams list
```

Choose on the team's actual remit, not on the repo name. **If it is not obvious, ask — do not
guess.** A project on the wrong team is more annoying to fix than to ask about.

## 3. Check it does not already exist

```bash
linear projects list --team <key>
```

If something close already exists, stop and ask. A duplicate project is worse than none: work
gets split across both and neither shows the real state.

## 4. Create it

```bash
linear projects create \
  --name "<name>" \
  --team <key> \
  --description "<description>" \
  --state planned \
  --icon "<emoji>"
```

Team keys (`ENG`) work anywhere a `--team` is taken. Note the project id from the response.

## 5. Report

Give the project URL, and say what you inferred and from where — so a wrong inference is
obvious and cheap to correct.

## Do not invent a backlog

The temptation is to follow up with placeholder issues: "Project setup", "Core implementation",
"Testing and documentation". Don't. They are indistinguishable from an empty backlog, they
carry no acceptance criteria, and they teach whoever reads the project that its issues are
noise. If real work is known, `/thmsn-jarvis` decomposes it properly; if it is not known, an
empty project is the honest state.
