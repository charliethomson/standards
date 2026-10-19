# TAV-38: LLM speaker selection for group chats
state=backlog priority=medium created=2026-09-17T10:56:24.684992720+00:00

Group chat's weakest link is turn-taking. `server/engine/src/prompt/group.rs` picks the next speaker by round-robin over non-muted members — so a character who was directly addressed by name waits their turn while an uninvolved one answers. Replace the ring with a cheap-model director that picks who *should* speak.

## Why this slots in cleanly

`next_speaker(members, history, selected)` already takes a `selected: Option<Id<Character>>` override that wins over round-robin (manual pick from the generate frame). The seam exists — LLM selection becomes another producer of `selected`, and the pure function stays pure and unit-tested.

## Approach

* New `GenerationKind` (director/speaker) so its spend is separately visible in the ledger, with its own model setting defaulting to something cheap and fast. This call sits *in front of* the reply the user is waiting on, so latency is the binding constraint — a slow selector makes every group turn feel worse.
* Input: recent turns (a short window — this doesn't need the whole history), the member list with their card blurbs, and who spoke last.
* Output: one member id plus a one-line reason. Constrain hard to the member set; a hallucinated name must fall back to round-robin rather than error.
* Make it opt-in per chat (a mode on the chat: round-robin | LLM director | manual), defaulting to today's behaviour so no existing chat changes underfoot.

## Failure modes to design against

* **Starvation.** An LLM selector will happily let one character dominate. Consider a floor — a member who hasn't spoken in N turns gets weighted up, or is excluded from being passed over again.
* **Latency.** If selection adds noticeable delay before the first token, it isn't worth it. Measure it; consider selecting speculatively during the *user's* typing rather than after send.
* **Muted members** must still be excluded — mute is a user instruction, not a hint.

## Acceptance

* Per-chat mode setting; default unchanged.
* Invalid/hallucinated selections fall back to round-robin silently.
* Selector spend visible as its own kind.
* Unit tests for the fallback paths; `group.rs` stays pure.
* `PARITY.md` updated.

---
_Imported from [THM-300](https://linear.app/thmsn/issue/THM-300/llm-speaker-selection-for-group-chats)._

