# prompts

Copy-paste prompts to hand an agent. These are **not** skills — once the submodule is
installed, the `thmsn-*` skills (`/thmsn-standards`, etc.) are auto-discovered and cover the
ongoing work. The prompts here are for the bootstrap and other one-offs you paste in yourself.

| Prompt | Use |
|---|---|
| [install.md](install.md) | Wire a repo to the standards submodule for the first time (before any skill exists). |
| [validate-install.md](validate-install.md) | Check that a repo's submodule install is correct and committed (PASS/WARN/FAIL). |
| [migrate-to-thmsn-ui.md](migrate-to-thmsn-ui.md) | Migrate a web app's local shadcn primitives onto the shared `@thmsn/ui` library, incrementally. |
| [linear-project-from-codebase.md](linear-project-from-codebase.md) | Infer a Linear project from a repo and create it. Bootstrap only — use `/thmsn-jarvis` to plan and decompose actual work. |

Once a repo is installed, you mostly won't need prompts — reach for the skills:
`/thmsn-standards` (audit/fix), `/thmsn-standards-review` (diff check),
`/thmsn-standards-init` (scaffold), `/thmsn-new-component`, `/thmsn-standards-sync`,
`/thmsn-standards-contribute`.
