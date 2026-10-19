# TAV-42: Auto-extract lorebook entries from play
state=backlog priority=high created=2026-09-17T10:56:25.255767428+00:00

Facts established during play are lost today. The rolling memory summary (`server/api/src/ws/memory.rs`) compresses them into prose that eventually decays, and nothing writes them back into the world. This turns play into durable world state: after a chat, propose new lorebook entries for what actually got established ("the innkeeper is named Bram", "the north road is flooded").

The world grows itself instead of being hand-authored up front. This is the feature with no SillyTavern equivalent worth copying.

## Approach

Reuse the studio machinery rather than building a subsystem. `server/api/src/ws/draft.rs` already streams an authoring conversation and splits a machine-readable JSON proposal off the reply at the `===DRAFT===` sentinel, with a per-`kind` system prompt in `server/api/prompts/`.

* Add a `kind` for lore extraction with its own prompt in `server/api/prompts/`.
* Input: the chat's message history (budget-clipped like the summariser's `MAX_TURNS`/`MSG_CLIP`), the character card, and — critically — **the lorebook entries that already exist**, so the model proposes *new* facts instead of restating the card.
* Output proposal: a list of candidate entries, each with `keys`, `content`, and a suggested `insertion_order` / `position`, plus the message ids the fact came from.
* Client shows a review sheet: accept / edit / reject per candidate. Nothing is written without an explicit accept — this is a proposal flow, never an automatic mutation.
* Accepted entries go through the existing `POST /lorebooks/{id}/entries`.

## Design questions to settle first

* **Trigger.** Manual ("extract lore from this chat") is the safe default. An automatic prompt after N turns risks nagging. Start manual.
* **Target book.** Which lorebook receives the entries when a chat has several linked, or none? Probably an explicit picker in the review sheet, with "create a new book for this chat" as an option.
* **Duplicate suppression.** Proposing a fact already in the book is the main failure mode. Feeding existing entries into the prompt handles the easy cases; exact-key collision should be filtered server-side regardless.
* **Key quality.** Extracted keys are what make the entry fire later, and bad keys are invisible failures — this pairs directly with the lorebook linter (see the linting issue).

## Acceptance

* Extraction runs from a chat, streams like any other draft, and produces reviewable candidates.
* No lorebook write happens without explicit per-candidate acceptance.
* Candidates cite the messages they came from.
* Spend attributed under its own kind in the ledger.
* `PARITY.md` updated; web first, then Apple/Windows per the feature flow.

---
_Imported from [THM-304](https://linear.app/thmsn/issue/THM-304/auto-extract-lorebook-entries-from-play)._

