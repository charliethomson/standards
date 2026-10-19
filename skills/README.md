# Shared skills

Claude Code skills shared across every repo in the fleet. One directory per skill, each
containing a `SKILL.md` (frontmatter `name` + `description`, then the body) plus any assets
it needs.

```
skills/
└── <skill-name>/
    ├── SKILL.md
    └── ...          # assets: served alongside the body
```

## How they reach a machine

The standards server (https://standards.thmsn.dev) serves every skill from this directory.
`standards doctor --fix` writes a **stub** per skill into `~/.claude/skills/<name>/`: the
frontmatter only, plus an instruction to call `get_skill("<name>")` on the `standards` MCP
server and follow the returned body. So:

- **The `description` is all a model sees before it fetches.** Keep it trigger-accurate: what
  the skill does and the phrases that should invoke it.
- **Bodies must read correctly with no local files.** Rules come from `get_doc` /
  `get_checklist`, templates from `get_template`, other skills are named by slash command
  (`/thmsn-ultron`), and doc links are absolute (`https://standards.thmsn.dev/d/<id>`).
- **Assets** (anything besides `SKILL.md`) are listed by `get_skill` and fetched with
  `get_skill_asset("<name>", "<path>")`, or over REST at
  `https://standards.thmsn.dev/api/v1/skills/<name>/assets/<path>`. A skill that runs a
  bundled script (`thmsn-logs`) materialises its assets into
  `~/.cache/thmsn/standards/skills/<name>/` first.

A change here reaches every machine once the server redeploys; the next `standards doctor`
refreshes any stub whose description changed. See
[skills-and-mcp](https://standards.thmsn.dev/d/skills-and-mcp) for the convention and
[migration](https://standards.thmsn.dev/d/migration) for moving a repo off the old submodule.

## Available skills

| Skill | What it does |
|---|---|
| **`thmsn-standards`** | Audit the current repo against the applicable standards and report, or fix, the findings. (`/thmsn-standards scan this repo and fix all findings`) |
| **`thmsn-standards-review`** | Diff-scoped check: flag only what your current changes/branch violate. A pre-commit/PR habit. |
| **`thmsn-standards-init`** | Bootstrap a new/empty repo to an archetype: scaffold the full layout from templates. |
| **`thmsn-new-component`** | Add a service/binary or a client surface (ios/macos/winui/web) to an existing product, on-standard. |
| **`thmsn-deep-review`** | Deep whole-repo audit (reads the source, one module at a time) → single dated markdown report `reviews/review-YYYY-MM-DD.md` → optional prioritized vision tasks. Not diff-scoped. Asset: `assets/review.template.md`. |
| **`thmsn-jarvis`** | Turn a rough idea into a scoped program: refine it with you, research, settle the decisions, file discrete vision tasks + a program brief. Hands off to `/thmsn-ultron`. (`/thmsn-jarvis we're going to work on a feature to…`) |
| **`thmsn-ultron`** | Run a multi-task program as an orchestrating manager: read the vision backlog, sequence it, dispatch implementer sub-agents, verify, keep the tracker current. Pausable/resumable across sessions. (`/thmsn-ultron work through the open tasks for this repo`) |
| **`thmsn-logs`** | Query the product's logs, metrics and traces in the homelab Grafana (Loki/Prometheus/Tempo) through its `scripts/obsq.sh` asset: error sweeps, trace lookup by `Ref:`, latency, what's live. Specialised per repo in `.standards.d/thmsn-logs/` (services, recipes, extra commands, notes). |
| **`thmsn-janitor`** | Find the worktrees and cargo target dirs agents left behind across `~/git`, say what's safe to delete, and clean up on your yes. Wraps `standards janitor`. |

## Jarvis → Ultron

Two halves of one workflow: **Jarvis plans, Ultron executes.** Jarvis's output (vision tasks
with blocking relations, plus `PROGRAM.md`) is exactly Ultron's input, so `/thmsn-jarvis` on a
vague idea and `/thmsn-ultron` on the resulting backlog is the intended path. Ultron handed an
unscoped idea will point you back at Jarvis rather than invent a decomposition.

In-flight state lives at `~/.local/state/thmsn/ultron/<program-slug>/`: a `PROGRAM.md` ledger
plus a volatile per-task journal, which is what makes a program pausable and resumable across
sessions. vision stays the durable record; that directory is reconstructible scratch.

Inspect it without spending tokens with `standards ultron`:

```sh
standards ultron ls                 # programs: task counts, what's blocked, last activity
standards ultron status <program>   # per-task table: state, plan progress, what each worker is on
standards ultron watch <program>    # live status, redrawn when the state changes (-n <seconds>)
standards ultron log <TASK-ID>      # a task's journal
standards ultron gc <program>       # drop state for a finished program (prompts)
```

## Janitor

Subagent worktrees are rarely removed, and each Rust one grows its own `target/`.
`standards janitor` sweeps every repo under `~/git`:

```sh
standards janitor scan                # read-only: every linked worktree, its size, and a verdict
standards janitor clean               # remove what the scan marked `remove` (prompts once)
standards janitor clean --targets     # also clear target dirs inside worktrees it keeps
standards janitor clean --branches    # also delete merged branches of removed worktrees
```

It only removes a worktree that is clean, unlocked, idle for `--days` (default 3), has no
process running in it, and whose HEAD is merged or still held by a branch.
