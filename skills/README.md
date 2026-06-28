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
| **`thmsn-standards-contribute`** | Author a change to the standards from a consuming repo and push it upstream, genericized. |

After adding or updating a skill here, consuming repos pick it up on their next
`standards/bin/standards sync` (which re-runs the linker).
