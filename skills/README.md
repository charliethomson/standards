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

> No shared skills yet — add the first one as a `<skill-name>/SKILL.md` here.
