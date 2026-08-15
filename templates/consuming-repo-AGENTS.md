<!--
  Copy this to ./AGENTS.md at the root of a repo that vendors the standards submodule.
  Replace {{PRODUCT}} and the archetype line. Keep it short — it is a pointer, not a copy.
-->
# AGENTS

This repo follows the shared engineering standards vendored at [`standards/`](standards/)
(a git submodule). **Read [`standards/AGENTS.md`](standards/AGENTS.md) first** — it is the
entrypoint and indexes every rule.

- **Archetype:** {{ARCHETYPE}} — see `standards/docs/archetypes/{{ARCHETYPE}}.md`.
- **Identifier:** `dev.thmsn.{{PRODUCT}}`
- Treat the standards as binding defaults. Repo-specific overrides (if any) are noted
  below; everything else defers to `standards/`.
- **[`README.md`](README.md) is the front door.** Change behaviour it describes — a command,
  the layout, what's generated, what's still a stub — and update it in the same commit. Ran
  a command from it that didn't work? Fix it before reporting back.
  ([`standards/docs/readmes.md`](standards/docs/readmes.md))

## Shortcuts — read before you go digging

[`SHORTCUTS.md`](SHORTCUTS.md) holds the answers to the operational questions that get
asked repeatedly here: which host is prod, what version is live, where the logs are.

- **Before** any environment recon — anything about the *running system* rather than the
  source — check it. The answer is often already one command.
- **After** any environment recon that cost more than a couple of commands, append what you
  learned, in the same turn. Record the **route** (endpoint, command, mechanism), never the
  **reading** ("prod is on 1.4.212" is stale by tomorrow).
- If a shortcut is wrong, fix it while you have the answer.

Convention: [`standards/docs/shortcuts.md`](standards/docs/shortcuts.md).

## Repo-specific overrides

_None yet. Document any intentional deviation from the standards here, with a reason._

## Keeping standards current

```sh
git submodule update --remote standards
```
