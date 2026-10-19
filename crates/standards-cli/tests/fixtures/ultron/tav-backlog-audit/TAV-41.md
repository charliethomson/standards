# TAV-41: Semantic retrieval: two-tier lore scanning + recall over archived history
state=backlog priority=high created=2026-09-17T10:56:25.132438568+00:00

Listed as deferred in `PLAN.md` §11 ("RAG / vector memory, TTS/STT, image generation — future"). This is the issue for the retrieval half.

Two distinct wins, one mechanism:

1. **Lore that fires on meaning, not spelling.** `server/engine/src/prompt/lorebook.rs` matches literal keys against recent messages. A paraphrase misses; the entry silently never fires and the model contradicts the world.
2. **Recall past the summary window** — the bigger win. Once turns fall out of the history budget (`prompt/budget.rs`) they exist only as compressed prose in the rolling memory. Retrieval over archived history gives genuine long-term recall.

## Blocking decision: where do embeddings come from?

**OpenRouter is not an embeddings provider.** This is the only idea in the set that adds a real dependency, and it must be settled before anything else:

* **Local model** (`fastembed-rs` / `candle` with bge-small or similar) — keeps the self-hosted promise intact, no second API key, works offline. Costs binary size, memory, and a model download on first run; ARM/x86 and Docker image implications.
* **Second cloud provider** — trivial to implement, but breaks "wraps OpenRouter" as the single upstream and adds a key to configure.

Recommendation: local, given the product is explicitly self-hosted. Validate the size/latency cost on the Docker image before committing.

## Approach once that's settled

* Storage: `sqlite-vec` extension, or a plain BLOB column plus in-process cosine — the corpus (one user's lore entries + chat history) is small enough that brute force is likely fine. Measure before reaching for an index.
* **Two-tier lore scanning:** keep keyword matching exactly as-is (deterministic, author-controlled, no surprises) and add semantic top-k with a similarity threshold. The two are complementary — never replace tier one with tier two.
* Embedding lifecycle: entries embed on write; chat history embeds in the background, in the same fire-and-forget slot as the memory refresh. Needs a backfill path for existing data and re-embedding when the model changes (store the model id alongside the vector).
* Retrieved chunks enter the prompt through the existing `LoreInjection` positions so budgeting and ordering are unchanged.

## Depends on

The prompt-trace work. Semantic retrieval is the single hardest feature here to debug blind — "why did this fire / why didn't it" is unanswerable without seeing which tier matched what, at what score. Do not start this before the trace ships.

## Acceptance

* Embedding provider decision recorded in `PLAN.md` before implementation.
* Keyword tier behaviour byte-identical to today.
* Trace shows tier, score, and threshold for every semantically-retrieved chunk.
* Backfill + re-embed path exists.
* Docker image size/startup impact measured and stated.

---
_Imported from [THM-303](https://linear.app/thmsn/issue/THM-303/semantic-retrieval-two-tier-lore-scanning-recall-over-archived-history)._

