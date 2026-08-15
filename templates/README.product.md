<!--
  Skeleton README for a product / service / orchestration repo.
  Convention and rationale: standards/docs/readmes.md

  Fill a section or delete it — a section left as a stub is worse than an absent one.
  Delete these comments as you go; the file is not done while any of them survive.
-->
<div align="center">

<img src="branding/brand/icon.svg" width="120" height="120" alt="{{PRODUCT}} mark" />

# {{PRODUCT}}

**<One line. What it does, in the words of someone who'd use it.>**

<!--
  Two to four sentences of what it actually is: the pieces, the surfaces, who talks to
  what. Enough that the layout table below reads as confirmation, not discovery.
-->

[Versioning](VERSIONING.md) · [Standards](AGENTS.md) · [Shortcuts](SHORTCUTS.md)

</div>

---

<!--
  If the repo is early, or a whole surface is a stub, say so HERE — it changes how
  everything below should be read:

  > **Early.** The server is real; every client is a stub. See [Known gaps](#known-gaps).
-->

## How it works

<!--
  The mechanism, not the feature list. What is the central noun, what happens to it, and
  which parts are independent of which?

  An ASCII diagram earns its place when there's a pipeline or a topology — something
  flowing through stages, or hosts resolving to each other. Two boxes is decoration.

  Then the properties a reader would otherwise discover by breaking something: what is
  independent of what, what is computed rather than stored, what backs off.
-->

## Layout

<!-- One line per top-level directory or crate — its role, not its contents. -->

| Path | What |
|---|---|
| `server/` | |
| `apps/web/` | |
| `api/` | |
| `branding/` | |
| `deploy/` | |
| `standards/` | Shared engineering standards, vendored as a submodule. Binding defaults |

## Quickstart

<!--
  Commands that work when pasted, in the order a newcomer runs them. Prerequisites first.
  Required env vars get a sentence saying WHY they're required — a server that refuses to
  boot without AUTH_ADMIN_KEY looks broken until you know it's refusing rather than
  failing. Show the loopback opt-out and its constraints too.
-->

```sh
```

## Development

<!-- The commands CI runs. Say so — that's what makes the section worth reading. -->

These are the same commands CI runs, so passing them locally means passing CI:

```sh
```

<!--
  If anything is generated — the OpenAPI spec, typed clients, brand tokens, icons — say
  what's generated from what, give the regeneration command, and state that CI fails on
  drift.
-->

## Things that will bite you

<!--
  Two to five real traps. The bar is "someone would plausibly lose an hour to this", not
  "this is worth knowing". Versions being derived rather than stored is usually one.
-->

## Known gaps

<!-- What isn't built, what's a stub, what's documented but absent. Delete if none. -->

## Deployment & observability

<!-- Pointers, three or four lines. Live state and ops routes go in SHORTCUTS.md. -->

## Docs

<!-- One line per doc. This is what makes the README a front door rather than a dead end. -->

- [`VERSIONING.md`](VERSIONING.md) — derived-version doctrine
- [`AGENTS.md`](AGENTS.md) — repo conventions and the [`standards/`](standards) submodule
- [`SHORTCUTS.md`](SHORTCUTS.md) — answers to the recurring operational questions
