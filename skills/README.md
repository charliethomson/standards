# Shared skills

Claude Code skills shared across every repo that vendors the `standards/` submodule. One
directory per skill, each containing a `SKILL.md` (plus any scripts/resources it needs).

```
skills/
└── <skill-name>/
    ├── SKILL.md
    └── ...
```

Consuming repos link these into their own `.claude/skills/` via
[`../templates/link-standards.sh`](../templates/link-standards.sh). See
[`../docs/skills-and-mcp.md`](../docs/skills-and-mcp.md) for the full convention.

## Available skills

| Skill | What it does |
|---|---|
| **`thmsn-standards`** | Audit the current repo against the applicable standards and report — or fix — the findings. (`/thmsn-standards scan this repo and fix all findings`) |
| **`thmsn-standards-review`** | Diff-scoped check: flag only what your current changes/branch violate. A pre-commit/PR habit. |
| **`thmsn-standards-init`** | Bootstrap a new/empty repo to an archetype — scaffold the full layout from templates. |
| **`thmsn-new-component`** | Add a service/binary or a client surface (ios/macos/winui/web) to an existing product, on-standard. |
| **`thmsn-standards-sync`** | Pull upstream standards, summarize what changed, and re-audit what's newly applicable here. |
| **`thmsn-standards-contribute`** | Author a change to the standards from a consuming repo and push it upstream, genericized. |
| **`thmsn-deep-review`** | Deep whole-repo audit (reads the source, one module at a time) → single dated markdown report `reviews/review-YYYY-MM-DD.md` → optional prioritized Linear tasks. Not diff-scoped. |
| **`thmsn-jarvis`** | Turn a rough idea into a scoped program — refine it with you, research, settle the decisions, file discrete Linear tasks + a program brief. Hands off to `thmsn-ultron`. (`/thmsn-jarvis we're going to work on a feature to…`) |
| **`thmsn-ultron`** | Run a multi-task program as an orchestrating manager — read the Linear backlog, sequence it, dispatch implementer sub-agents, verify, keep the tracker current. Pausable/resumable across sessions. (`/thmsn-ultron work through the open tasks for this repo`) |

## Jarvis → Ultron

Two halves of one workflow: **Jarvis plans, Ultron executes.** Jarvis's output — Linear tasks
with blocking relations, plus `PROGRAM.md` — is exactly Ultron's input, so `/thmsn-jarvis` on a
vague idea and `/thmsn-ultron` on the resulting backlog is the intended path. Ultron handed an
unscoped idea will point you back at Jarvis rather than invent a decomposition.

In-flight state lives at `~/.local/state/thmsn/ultron/<program-slug>/` — a `PROGRAM.md` ledger
plus a volatile per-task journal, which is what makes a program pausable and resumable across
sessions. Linear stays the durable record; that directory is reconstructible scratch.

Inspect it without spending tokens using [`../bin/ultron`](../bin/ultron):

```sh
ultron ls                 # programs: task counts, what's blocked, last activity
ultron status <program>   # per-task table: state, plan progress, what each worker is on
ultron watch <program>    # live status, redrawn when the state changes (-n <seconds>)
ultron log <TASK-ID>      # a task's journal
ultron gc <program>       # drop state for a finished program (prompts)
```

Symlink it once: `ln -sfn "$PWD/standards/bin/ultron" ~/.local/bin/ultron`.

After adding or updating a skill here, consuming repos pick it up on their next
`standards/bin/standards sync` (which re-runs the linker).
