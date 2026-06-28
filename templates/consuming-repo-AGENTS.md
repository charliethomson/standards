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

## Repo-specific overrides

_None yet. Document any intentional deviation from the standards here, with a reason._

## Keeping standards current

```sh
git submodule update --remote standards
```
