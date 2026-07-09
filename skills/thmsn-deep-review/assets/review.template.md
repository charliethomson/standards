# {{PRODUCT}} code review — {{DATE}}

> Whole-repo deep audit across correctness, security, architecture, and testing.
> {{N}} findings ({{C}} critical · {{H}} high · {{M}} medium · {{L}} low · {{NIT}} nit) across {{MODULE_COUNT}} modules.
> Overall grade: **{{OVERALL_GRADE}}**.

{{Executive summary — 2–4 sentences. The single most important takeaway, the strongest and
weakest parts, and whether anything is a "fix now".}}

## Scorecard

| Module | Lang | LOC | Grade | C / H / M / L / N | Top risk |
|---|---|---:|:---:|:---:|---|
| [server/core](#servercore) | Rust | 920 | B | 0/0/3/3/2 | short phrase |
| … | | | | | |

**Totals:** {{N}} findings — {{C}} critical, {{H}} high, {{M}} medium, {{L}} low, {{NIT}} nit.

## Architecture

```
{{ASCII map of how the modules relate — sources of truth, data flow, which layer owns what.}}
```

{{One paragraph on the shape of the system and any notes: what's generated vs hand-written,
what was line-audited vs skimmed.}}

## Cross-cutting themes

The highest-value findings recur across modules — fix them at the source and several
per-module findings close at once. A theme belongs here only when the *same root cause*
appears in ≥2 modules.

### {{Theme}} — modules: {{a, b, c}}

{{What the shared gap is, where it surfaces in each module (`file:line`), and the single fix.}}
→ {{ISSUE-REF if created}}

## What to fix first

| P | Fix | Where | Why now | Issue |
|:---:|---|---|---|---|
| P0 | … | module | … | {{ISSUE-REF}} |
| P1 | … | | | |

## Module reviews

### server/core

**Grade: B** · Rust · 920 LOC · one-line description.

{{2–3 sentence summary: role, health, main takeaway.}}

- **[Med]** Short title — `server/core/src/id.rs:101` — what's wrong and why. Fix: … → {{ISSUE-REF}}
- **[Low]** … — `file:line` — … Fix: …

**What's good:** {{genuine strengths — verified, not assumed.}}

**Testing & CI:** {{coverage state, gaps, what to add.}}

### {{next module}}

…

## Linear issues created

{{Only if issues were filed. Group by priority; one line each. Omit this whole section if none.}}

**P0 · Urgent**
- `ABC-154` [macOS self-updater: verify code signature before swap](https://linear.app/issue/ABC-154) — Security, Bug
- …

**P1 · High**
- …

**P2 · Medium** — …

**P3 · Low** — …

Relations: `ABC-147` blocks `ABC-148`, `ABC-150`; `ABC-153` blocks `ABC-174`; …

## Methodology

One deep-audit subagent per module, each reading the source and citing `file:line`, across
correctness & bugs, security, architecture & maintainability, and testing & CI. Generated
code ({{list}}) was noted but not line-audited. Severities: **Critical** = exploitable /
data-loss / crash · **High** = real bug or security gap · **Medium** = likely bug or design
flaw · **Low** = minor · **Nit** = style.
