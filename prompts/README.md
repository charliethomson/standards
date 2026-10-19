# prompts

Copy-paste prompts to hand an agent. These are **not** skills: once `standards doctor --fix`
has wired a machine, the `thmsn-*` skills (`/thmsn-standards`, etc.) are available in every
repo and cover the ongoing work. The prompts here are for one-offs you paste in yourself.
They are not served by https://standards.thmsn.dev.

| Prompt | Use |
|---|---|
| [migrate-to-thmsn-ui.md](migrate-to-thmsn-ui.md) | Migrate a web app's local shadcn primitives onto the shared `@thmsn/ui` library, incrementally. |
| [vision-project-from-codebase.md](vision-project-from-codebase.md) | Infer a vision project from a repo and create it. Bootstrap only: use `/thmsn-jarvis` to plan and decompose actual work. |

Setting up a machine or moving a repo off the old `standards/` submodule is no longer a
prompt: follow the [migration runbook](https://standards.thmsn.dev/d/migration).

For ongoing work, reach for the skills: `/thmsn-standards` (audit/fix),
`/thmsn-standards-review` (diff check), `/thmsn-standards-init` (scaffold),
`/thmsn-new-component`.
