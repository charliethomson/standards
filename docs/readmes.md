# READMEs

Every repo has a root **`README.md`**, committed, written for someone who has never seen the
repo before — human or agent — and who needs to understand the thing and run it *today*.

> If a reader has to open three files to learn what the repo is, the README failed.

It is the only document in the repo with that audience. Everything else assumes context.

## Three files, three jobs

Don't let them bleed into each other. Each has a different reader arriving with a different
question:

| File | Answers | Reader |
|---|---|---|
| **`README.md`** | *What is this, how does it work, how do I run it?* | anyone arriving cold |
| **`AGENTS.md`** | *What rules bind a change I'm about to make?* | an agent about to edit |
| **`SHORTCUTS.md`** | *Where is the running system and how do I ask it?* | an agent doing ops recon |

A README that explains the deploy mechanism in detail is doing `SHORTCUTS.md`'s job; one
that lists coding conventions is doing `AGENTS.md`'s. Link to them instead — see
[`shortcuts.md`](shortcuts.md).

## Where READMEs live

- **Root** — mandatory, every repo, every archetype. No exceptions.
- **Every independently-buildable subtree** — `server/`, each `apps/<client>/`, `branding/`,
  `api/`, `deploy/`. Its own README when it has its own build, its own commands, or its own
  generated artefacts. The root links to them and does not repeat them.
- **Orchestration repos** — one per stack directory, covering that stack's operational
  detail; the root carries the inventory table and the cross-cutting schemas (hostnames,
  DNS, topology).

The root README owns the *shape of the whole*; a sub-README owns the *detail of its part*.
When they disagree the root is wrong, because it is the one that goes stale.

## The spine

Sections in this order. Skip any that don't apply to the repo — but skip them, don't stub
them.

### 1. Header

Name, then a **single bold line** saying what it does in the user's words, then two to four
sentences of what it actually is: the pieces, the surfaces, who talks to what. Enough that
the layout table below reads as confirmation rather than discovery.

Products with a brand mark centre the header and show the mark from `branding/`
(see [`branding.md`](branding.md)); libraries and tools use a plain `# name` and skip
straight to the pitch. A row of links to the neighbouring docs
(`Versioning · Standards · Branding · Contract`) belongs here; badges are optional and
never the first thing.

The repo's name is its title. Not a prettified variant — the reader arrived from a
directory listing or a git URL and needs the names to match.

### 2. How it works

**The mechanism, not the feature list.** This is the section that earns the README and the
one most often missing. What is the central noun, what happens to it, and which parts are
independent of which?

Reach for an ASCII diagram when there is a **pipeline or a topology** — something flowing
through stages, or hosts resolving to each other. Skip it when the answer is a paragraph;
a diagram of two boxes is decoration.

Then call out the properties a reader would otherwise have to discover by breaking
something: what is independent of what, what is adaptive, what is computed rather than
stored.

### 3. Layout

One line per top-level directory or crate, saying its *role* — not its contents. A table
for a workspace of many crates, a fenced tree for a handful of directories; either way,
every entry earns its line by telling the reader where to go next.

Mark the ones that aren't what they look like: a subtree deliberately excluded from the
workspace, a directory of throwaway experiments, the vendored `standards/` submodule.

### 4. Quickstart

Commands that work when pasted, in the order a newcomer runs them. Prerequisites first
(and *why* they aren't bundled, if that's a question worth pre-empting).

**Required environment variables get a sentence saying why they're required.** This is
where the fleet's fail-closed auth surprises people: a server that refuses to boot without
`AUTH_ADMIN_KEY` looks broken until you know it is refusing rather than failing. Show both
the real-auth invocation and the loopback opt-out, with its constraints. See
[`auth-integration.md`](auth-integration.md).

### 5. Development

**The commands CI runs**, stated as such — "passing these locally means passing CI" is the
sentence that makes the section worth reading. Group by part (Rust workspace, JS workspace,
each client) and keep them copy-pasteable.

Then the **generated-artefact rule**, if the repo has one: what is generated from what, the
regeneration command, and the fact that CI fails on drift. Every full-stack product in the
fleet has this and every one of them has been broken by someone hand-editing a generated
file. See [`contracts.md`](contracts.md).

### 6. Things that will bite you

Two to five items, each a genuine trap: the type mirrored across three languages, the
package that can't take the obvious name because a system framework owns it, the version
field that looks editable and isn't. The bar is *someone would plausibly lose an hour to
this*, not *this is worth knowing*.

### 7. Status and known gaps

**Say what isn't built.** A README that describes the intended system as though it exists
costs the next reader a day. When a repo is early, or a surface is a stub, or a documented
artefact is empty — write it down, plainly, near the top if it changes how the whole repo
should be read.

This is a fleet trait worth keeping: it is what makes the rest of the README trustworthy.

### 8. Deployment and observability

Three or four lines of pointers — what deploys it, what fronts it, where logs and metrics
go — each linking to the real thing. Not the runbook, and not the current state of prod:
routes to the running system belong in [`SHORTCUTS.md`](shortcuts.md).

### 9. Docs

A short index of the repo's other docs, one line of what each covers. This is what makes
the README a front door rather than a dead end.

Close with the pointer to `AGENTS.md` and the vendored [`standards/`](../README.md)
submodule.

## Per-archetype

| Archetype | Additionally required | Not expected |
|---|---|---|
| **Full-stack product** | Contract/codegen section; per-client build table; sub-READMEs per app | — |
| **Library** | Install snippet with the correct git URL scheme; a *minimal working* example; feature-flag table; what stays in the consuming repo; coverage command | Deployment, branding, clients |
| **CLI / service tool** | Install one-liner from the registry (and that re-running it upgrades); a subcommand tour; config/completions | Client tables |
| **Orchestration** | Stack inventory table; hostname/DNS schema; network topology; **state that lives outside the repo** | Quickstart (there is no "run it locally") |
| **Vendored app** | What upstream is and the pinned ref; what we add and where; that there is no rebrand | Architecture of upstream's code |

For libraries, the README **is** the API documentation
([`archetypes/library.md`](archetypes/library.md)) — which raises the bar for the example,
not the length. Use the right dependency scheme: HTTPS for public repos, SSH for private
([`rust-conventions.md`](rust-conventions.md)).

## Style

- **State the mechanism, not the state.** Same rule as shortcuts: "Watchtower polls every
  ~5m" stays true; "prod is on 1.4.212" is wrong by tomorrow. Version numbers, dates, and
  counts of things do not belong in a README.
- **Prose for mechanisms, tables for enumerations.** A table of crates is readable; a table
  of ideas is not.
- **Every command runs as written.** No `<placeholder>` where a real value would do, no
  implied `cd`.
- **No hand-maintained table of contents.** It rots within two edits, and GitHub renders one
  from the headings for free.
- **Link with relative paths** so they're clickable on the forge and in a checkout: a
  markdown link to `server/README.md`, never a bare mention or an absolute URL.
- **Plain declarative prose, no marketing.** "The scheduler polls it and backs off when the
  account is absent", not "leverages an adaptive polling engine".

## Size

Target **under ~200 lines**. Past that, the reader stops reading and the tail stops being
maintained.

When a section outgrows the README, **promote it to `docs/` and leave a one-line pointer**.
A 500-line README is a document that lost track of its audience: someone deciding whether
to use the thing, and someone trying to run it. Reference material — protocol definitions,
migration guides, design notes — has a different reader and belongs in its own file.

## What stays out

- **Anything in `standards/`.** Link to it. The README says *this repo commits its OpenAPI
  spec and CI fails on drift*; it does not restate the contracts standard.
- **Secrets.** Env var *names*, never values. Same rule as [`deployment.md`](deployment.md).
- **Coding conventions** — `AGENTS.md`.
- **Operational routes and live state** — `SHORTCUTS.md`.
- **Historical design notes and plans** — `docs/`, linked from the index.
- **Aspiration written as fact.** If it isn't built, the sentence says so.

## Keeping it true

**Change behaviour the README describes, update the README in the same commit.** The parts
that rot, in order: the commands (a script gets renamed), the layout (a crate is added), the
generated-artefact list, and the status section (a stub gets implemented and stays labelled
a stub).

If you are an agent and you ran a command from the README that didn't work, fix the README
before you report back — you are in the one context that knows the correct command.

## Scaffolding

`standards install` writes a skeleton from
[`../templates/README.product.md`](../templates/README.product.md) (or
[`../templates/README.library.md`](../templates/README.library.md) for the library and CLI
archetypes), and `standards sync` backfills it for repos installed before this convention
existed. Neither ever clobbers an existing README.

The skeleton is a skeleton: it names the sections and explains each in an HTML comment you
delete as you fill it. A repo that ships the skeleton unfilled is worse off than one with no
README, because the skeleton looks like documentation.

## Checklist

- [ ] `README.md` exists at the repo root and is committed.
- [ ] Opens with the repo's own name, a one-line pitch, and what it actually is.
- [ ] Explains the **mechanism** — not just a feature list.
- [ ] Layout section maps every top-level directory to a role.
- [ ] Quickstart commands run as pasted; required env vars explain *why* they're required.
- [ ] Development section states the commands CI runs, and the regeneration command for any
      generated artefact.
- [ ] Anything stubbed, missing, or broken is stated as such.
- [ ] Every independently-buildable subtree has its own README, linked from the root.
- [ ] No secrets, no live state, no duplication of `standards/`, no hand-written ToC.
- [ ] Under ~200 lines; overflow lives in `docs/` with a pointer.
