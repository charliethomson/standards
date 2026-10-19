# TAV-39: Character & lorebook linter
state=backlog priority=low created=2026-09-17T10:56:24.849482584+00:00

A one-shot analysis pass over a character card or lorebook that reports what will go wrong before it goes wrong. Directly serves the "better than SillyTavern" positioning: ST lets you author a card that silently never works and gives you no way to find out.

## Checks

Split by whether they need a model at all — several don't, and those should land first because they're free and deterministic.

**Static (no model call):**

* Keys that can never fire — an entry whose keys never appear in any plausible message, or that collide with another entry.
* Keys so broad they fire on *every* message (articles, the character's own name) — these silently eat the book's token budget.
* Token bloat: per-entry and whole-book estimates against the budget, using the existing `prompt/budget.rs` estimator. Show what will be dropped first.
* Entries disabled or orphaned (linked to no book, or a book linked to no character).
* Empty/whitespace fields on a card that the prompt renderer will skip.

**Model-assisted:**

* Internal contradictions between description / personality / scenario.
* Description vs example-dialogue voice mismatch.
* Lore entries that contradict the card, or each other.

## Approach

Reuse the studio's propose-and-review shape (`server/api/src/ws/draft.rs`, per-kind prompt in `server/api/prompts/`) so findings arrive as structured JSON the client can render as a list — each finding with a severity, the field or entry it points at, and where applicable a suggested fix the user can apply with one click.

Findings are advisory. Nothing is auto-applied.

## Notes

* Static checks are worth shipping standalone even if the model-assisted half never happens — they're cheap, instant, and catch the most common real failure (over-broad keys).
* Pairs with lore extraction: extracted entries are exactly the ones whose keys are most likely to be badly chosen, so run the linter over extraction output before accepting.

## Acceptance

* Static checks run with no model call and no spend.
* Findings are structured, severity-ranked, and point at a specific field/entry.
* No mutation without explicit user action.
* `PARITY.md` updated.

---
_Imported from [THM-301](https://linear.app/thmsn/issue/THM-301/character-and-lorebook-linter)._

