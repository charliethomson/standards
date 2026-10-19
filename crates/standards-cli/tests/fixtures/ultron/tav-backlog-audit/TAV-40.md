# TAV-40: Message classification: repetition flags, scene beats, expression portraits
state=backlog priority=medium created=2026-09-17T10:56:24.999227409+00:00

Derive per-message signals so the UI can react to *what a reply is*, not just its text. Classification on its own is a toy — each phase below has to pay for itself with a visible feature.

Ships in three independent phases, cheapest first. Each is landable alone.

## Phase 1 — Repetition / slop detection (no model call)

Long roleplay's #1 complaint is the model looping. This needs no LLM at all.

* On each completed assistant reply, compute n-gram overlap (3–5 grams, normalised case/punctuation) against the last N assistant turns in the same chat.
* Store a `repetition_score` on the message; flag above a tuned threshold.
* Web surfaces a subtle badge on the message with a one-click regenerate (reuses the existing swipe path in `server/api/src/ws/generate.rs`).
* Pure logic → put it in `server/engine/` as a unit-tested submodule alongside `prompt/budget.rs`.

Open question: whether the score is computed inline (blocks nothing, it's cheap) or in the same fire-and-forget slot as `memory::maybe_refresh`.

## Phase 2 — Scene / beat detection, piggybacked on the memory summariser

Chats of 800+ messages are unnavigable today. This gives a timeline without a single extra model call.

* `server/api/src/ws/memory.rs` already runs a summariser every `REFRESH_EVERY = 10` messages. Extend `server/api/prompts/memory.md` to *also* emit, alongside the prose summary, a short beat label for the span and a boundary flag ("did a new scene start, and at which message").
* Persist beats as their own rows keyed to the message that opened them.
* Web renders them as markers in the chat scrollbar / a jump-to-beat list.
* Cost: zero marginal model spend — it rides an existing `GenerationKind::Memory` call.
* Care needed: the summariser prompt currently returns prose. Moving it to a structured envelope means versioning the prompt and tolerating models that ignore the format (fall back to prose-only, no beats).

## Phase 3 — Emotion classification → portrait swap

SillyTavern's expression packs, done properly. Character image galleries already exist (`/characters/{id}/images`).

* Classify the affect of each assistant reply into a small fixed set (neutral / happy / sad / angry / surprised / afraid — keep it small, big taxonomies classify badly and are hard to author art for).
* Requires an expression → image mapping on the character (tag gallery images with an expression slot).
* Swap the portrait as the reply streams in; fall back to the primary avatar when a slot is unmapped.
* Needs a new `GenerationKind` so the spend shows up separately in the ledger (`server/db/src/generations.rs`) — use a cheap model by default, with its own model setting like the existing per-kind settings.

## Explicitly out of scope

Content-safety / moderation classification. Tavern is single-user and self-hosted; there is nobody to moderate for.

## Acceptance

* Phase 1 lands with unit tests over the n-gram scorer and a visible badge in web.
* Phase 2 degrades cleanly to today's behaviour when the model ignores the structured envelope.
* Phase 3 spend is separately attributable in the spend explorer.
* Parity table (`PARITY.md`) updated in the same change per the feature flow.

---
_Imported from [THM-302](https://linear.app/thmsn/issue/THM-302/message-classification-repetition-flags-scene-beats-expression)._

