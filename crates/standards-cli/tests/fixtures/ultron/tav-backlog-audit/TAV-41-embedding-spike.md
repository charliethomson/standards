# TAV-41 embedding spike — measured cost of a local model

Date: 2026-09-30. Spike branch (local only, not pushed): `spike/tav-41-embeddings`
in worktree `~/git/tavern/.claude/worktrees/agent-a5255f178e4b7c269`
(commits `aec7705`, `f6c102e` on top of `e62c16d`).

This follows the July spike (`reviews/spike-embeddings-2026-07-30.md`,
`spikes/embeddings/`, commits 849c3e4 / 2ba67fa), which left the Docker, RSS,
latency and cosine numbers as estimates. Every number below is tagged
**NEW** (measured today) or **JULY** (carried over).

## TL;DR

- **The item's premise is stale.** TAV-41 says OpenRouter has no embeddings
  API. It does: `POST /api/v1/embeddings`. The July spike found this, and I
  re-checked it today: `openai/text-embedding-3-small` is $0.02/M tokens and
  `baai/bge-base-en-v1.5` is $0.005/M. A cloud backend therefore needs **no second key and no
  second provider**.
- **A local model works, and the cost is moderate:** about +46 MB on the image, about +200 MiB
  idle RSS, 55–60 ms to embed a lore entry on 2 threads, and +12 % on the cold build. The catch is that
  **fastembed/ort cannot link on the current bookworm image**. The prebuilt
  ONNX Runtime needs glibc ≥ 2.38, so the base image has to move to trixie.
- **Brute-force cosine is fine.** It takes 0.7 ms over 5k vectors and about 7 ms over 50k. No sqlite-vec.
- **My recommendation is unchanged from July, but for a different reason:** start with a
  remote OpenAI-compatible backend (OpenRouter by default, or a self-hosted
  endpoint) behind an `EmbeddingBackend` trait. Keep `Local` (fastembed) as a
  later, optional cargo feature. The deploy box no longer rules out a local model. The reason not to
  build it first is the engineering and ops cost: a base-image migration, a model-download lifecycle,
  an AVX2 floor and a slower CI build. And "works offline" is no advantage, because
  Tavern cannot generate anything offline anyway: every completion already goes to OpenRouter.

## What was built (spike branch)

- `server/engine/src/embeddings.rs`, behind the `embeddings` feature. It wraps `fastembed 7.1.0`
  (`default-features = false`, `ort-download-binaries-rustls-tls` +
  `hf-hub-rustls-tls`) and bge-small-en-v1.5 in two variants: fp32 (`Xenova/…`) and int8
  (`Qdrant/…-onnx-Q`). It also has `cosine`, a brute-force `top_k` and a `to_blob` (LE f32).
- `server/api`: feature `embeddings` forwards to the engine. With the feature on, `main.rs`
  loads the model at startup via `spawn_blocking`, using 2 intra-op threads and
  `$TAVERN_EMBEDDINGS_CACHE` (default `<data_dir>/models`).
- `server/engine/src/bin/embed_bench.rs` (`required-features = ["embeddings"]`)
  measures load time, RSS, a single embed, a 100-message batch and top-k.
- `server/Dockerfile`: new build args `DEBIAN` (bookworm|trixie) and `CARGO_FEATURES`, plus a
  `bench` target.
- `spikes/tav41-candle/`: a standalone candle 0.11 BERT bench on the same model and workload.
  This is the alternative I evaluated in case fastembed hit a blocker.

## Environment

- Deploy box, checked read-only over ssh today (**NEW**): `thmsn-chunk-dev`, x86_64,
  Ryzen 9 5950X (**AVX2, no AVX-512/VNNI**). It has **12 vCPU and 16 GiB RAM (8.3 GiB
  available)**, 4 GiB swap (1.3 GiB used) and **108 GB free disk (43 %)**. `tavern-api:main`
  is 123 MB, and the running api container uses 209 MiB. This differs a lot from July's
  inventory (4 vCPU, 7.8 GiB, 3 GiB swapped, 92 % disk full).
- Measurement host: M4 Pro Mac, OrbStack. linux/arm64 containers run natively. The linux/amd64 builds
  (for image size) and the amd64 runs go through Rosetta, so their **timings are not representative**
  of the Zen 3 box.
- I did **not** run anything on the deploy box. It is a shared production host, and this
  task did not authorise pulling images or loading its CPU there.

## Numbers

### Docker image (linux/amd64, same Dockerfile, `--ssh`, cold `--no-cache`) — NEW

| Image | Bytes | api binary | vs prod today |
| --- | --- | --- | --- |
| bookworm, no feature (current prod recipe) | 122.6 MB | 33.3 MB | — (matches ghcr `:main` 123 MB) |
| bookworm + `embeddings` | **link failure** | — | — |
| trixie, no feature | 136.6 MB | 33.5 MB | +14.0 MB (base image only) |
| trixie + `embeddings` | **168.8 MB** | **65.8 MB** | **+46.2 MB** (+32.3 MB binary, +14 MB base) |
| + int8 model baked in (arithmetic: + cache dir) | ~235 MB | | +112 MB |
| + fp32 model baked in (arithmetic) | ~302 MB | | +179 MB |

The bookworm blocker is `rust-lld: undefined symbol: __isoc23_strtoll` and
`std::__cxx11::basic_string::_M_replace_cold`. ort 2.0.0-rc.13's prebuilt static
ONNX Runtime was built against glibc ≥ 2.38 and GCC 13's libstdc++, and bookworm
has glibc 2.36 and GCC 12. The fixes are:

1. Move build and runtime to trixie (measured above: +14 MB). This is what I did.
2. Stay on bookworm with `ort-load-dynamic` and ship Microsoft's
   `libonnxruntime.so` in the image. Not measured.
3. Use candle, which has no C++ runtime. I verified that it builds and runs on bookworm.

The binary is statically linked against ONNX Runtime; the only new runtime
dependency is `libstdc++.so.6`, which trixie-slim already has. The ort prebuilt also
**requires AVX2**: it prints a warning and would SIGILL without it. The deploy box has AVX2, but
older self-host CPUs may not.

### Model download / cache — NEW (sizes also JULY, confirmed)

| | int8 (`Qdrant/bge-small-en-v1.5-onnx-Q`) | fp32 (`Xenova/bge-small-en-v1.5`) |
| --- | --- | --- |
| Download (onnx + tokenizer) | 66.5 MB + 0.7 MB (65 M on disk) | 133.1 MB + 0.7 MB (128 M) |
| First start (download + load), arm64 container | 3.3–3.9 s | 5.2–6.1 s |
| Warm load from cache | 0.06–0.1 s | 0.1–0.3 s |

- The cache uses the hf-hub layout (`models--<org>--<name>/blobs|snapshots`). **fastembed's
  default is `./.fastembed_cache` relative to the CWD.** In the container that is `/app`,
  which is not a volume, so every Watchtower recreate would re-download the model. It has to be pointed at
  `/data/models`, which the spike does.
- Fetching on first use requires network access to huggingface.co (or `HF_ENDPOINT` for a mirror).
  An offline self-host would need a pre-seeded `/data/models`, the weights baked into the image, or
  `TextEmbedding::try_new_from_user_defined` with bytes from disk. As noted in the TL;DR, a Tavern
  install without network access cannot generate anyway.
- Build-time network dependency: ort's build script downloads the prebuilt ORT
  (about 77 MB unpacked, from the pyke CDN) during `cargo build`.

### RSS with the model loaded — NEW

These are process RSS figures from the bench (VmRSS). The process baseline before loading is about 8 MiB. Today the prod api
container uses 209 MiB without a model, so the server with the model would be roughly that plus the delta.

| linux/arm64 container, 2 threads | int8 | fp32 |
| --- | --- | --- |
| After load | 108 MiB | 217–221 MiB |
| After first inference (**idle, model resident**) | **195 MiB** | **218–222 MiB** |
| After a 100-message batch, default batch (256) | 577–581 MiB | 400–402 MiB |
| After a 100-message batch, `batch_size = 8` | **298 MiB** | **232 MiB** |

- ORT's arena grows to the largest batch × sequence it has seen and never shrinks.
  **Batch size matters more than model choice.** July said the same (JULY estimate; now confirmed).
  Cap backfill at batch 8.
- int8 has a higher peak than fp32 because the quantised graph allocates more activation buffers. On
  arm64/Linux it is also no faster (next table), so int8 only saves disk and download.
- For comparison, candle (fp32, mmap'd safetensors) peaks at 147–202 MiB. Most of that is
  file-backed pages the kernel can drop, whereas ORT's is anonymous heap.

### Embedding latency, p50 — NEW

The lore entry is about 200 tokens (the bench's `LORE` constant). The batch is 100 chat messages of about 35–45 tokens each.

| Where | Engine / threads | 1 lore entry | 100 messages |
| --- | --- | --- | --- |
| Mac M4 Pro native | ort fp32, 2 thr | 33 ms | 553 ms |
| Mac M4 Pro native | ort fp32, all cores | 59–77 ms (noisy) | 591–624 ms |
| Mac M4 Pro native | ort int8, 2 thr | 20 ms | 380 ms |
| Mac M4 Pro native | ort int8, all cores | 22–23 ms | 270–286 ms |
| linux/arm64 container `--cpus 4` | ort fp32, 2 thr | **56–59 ms** | **1.54–1.68 s** |
| linux/arm64 container | ort fp32, all cores (14) | 27 ms | 500 ms |
| linux/arm64 container `--cpus 4` | ort int8, 2 thr | 59–60 ms | 1.57–1.67 s |
| linux/arm64 container | ort int8, all cores | 38 ms | 486 ms |
| linux/amd64 via Rosetta `--cpus 4` | ort fp32 / int8, 2 thr | 110 / 116 ms | 3.2 / 3.3 s (emulated; upper bound only) |
| Mac native | candle fp32, 10 thr | 89 ms | 1.60 s |
| linux/arm64 container `--cpus 4` | candle fp32 | 118 ms | 2.10 s |

Reading the table:
- A per-generation query embed (short text) is well under 60 ms on 2 threads, which is small next to
  time-to-first-token. July estimated 10–25 ms for a 60-token query (JULY), which is plausible; I measured a
  200-token entry.
- Backfill at 2 threads is about 16 ms per message, so 10k messages take about 3 minutes and 50k about 14 minutes (arm64
  container). Zen 3 with AVX2 should be in the same range but **is not measured**. The Rosetta
  numbers are an upper bound. July's estimate for bge-small was 6–8 minutes per 10k (JULY).
- candle is 2–4× slower than ORT. In exchange it has a 7.8 MB binary (vs +32 MB), builds on bookworm, and has
  no AVX2 floor.
- Both engines give identical vectors: cos(lore, paraphrase with no shared
  keywords) = 0.657, and cos(lore, an unrelated sentence) = 0.367. The threshold will need tuning because the
  unrelated floor for bge is high.

### Brute-force cosine top-8, 384-dim, in process (naive scalar loop) — NEW

| Corpus | Matrix size | Mac native | arm64 container | amd64 Rosetta |
| --- | --- | --- | --- | --- |
| 5,000 | 7.3 MiB | 0.68 ms | 0.67–0.70 ms | 0.94 ms |
| 50,000 | 73.2 MiB | 6.5–6.8 ms | 6.8–7.0 ms | 9.4–9.6 ms |

A plain BLOB column with in-process cosine is fine. One user's lore plus per-chat history is
far below 50k per query scope, and even a whole-account scan is under 10 ms. The 50k scan is memory-bandwidth bound: it
reads 73 MB. July said "well under a millisecond" for 50k, and the measurement shows that
was optimistic by about 10×, though the conclusion (no sqlite-vec) still holds. These timings exclude the cost of reading or decoding
BLOBs out of SQLite, which I did not measure.

### Cold build time (CI shape: amd64, `CARGO_BUILD_JOBS=2`) — NEW

| | cargo `Finished` |
| --- | --- |
| bookworm baseline (sequential, uncontended) | 12 m 55 s |
| bookworm + embeddings | reached the final link at 14 m 50 s, then failed (see above) |
| trixie baseline (run in parallel with the next row) | 14 m 03 s |
| trixie + embeddings | 15 m 46 s |

The feature adds **about 1 m 45 s (+12 %)** and 56 crates, including ort-sys, tokenizers with onig (C),
ndarray, hf-hub/ureq and rayon. All of this ran under Rosetta, so absolute times overstate the
amd64 CI agents. The ratio is the useful number. The cargo step is a single layer, so any
server change pays the extra time. (The spike's own `embed-bench` binary adds another 3.5 minutes,
because feature unification makes it re-compile ort-sys. That cost is specific to the spike and would not ship.)

## Cloud alternative (not implemented)

- **OpenRouter `/api/v1/embeddings`** (OpenAI-compatible). It reuses the existing key, the
  `server/openrouter` client and the spend ledger (a new `GenerationKind::Embedding`; see
  JULY §4). Prices checked today (**NEW**):
  - `openai/text-embedding-3-small`: $0.02/M tokens. It returns 1536 dims and supports the
    `dimensions` param to shrink them.
  - `baai/bge-base-en-v1.5`: $0.005/M.
  - `qwen/qwen3-embedding-8b`: $0.01/M.
  - A 1.5 M-token history backfill costs about $0.03.
- Latency: I did not measure it, because I did not use a key. Expect a network round trip of roughly 100–400 ms per call from
  the homelab. That is slower than local, but it can be batched and moved off the hot path.
- Costs of this option: chat content (including a whole-history backfill) goes to OpenRouter and the routed
  provider. Training can be opted out of; retention cannot be pinned (JULY §4).
  If the call fails, generation falls back to keyword-only lore.
- A self-hosted OpenAI-compatible endpoint (TEI, Infinity or Ollama on another guest) uses the
  same code path, with no third party involved (JULY option D).

## Recommendation

**Do not build Local first.** Ship `EmbeddingBackend` with `None` (the default) and
`Remote { base_url, api_key, model }` implementations. Default the remote to OpenRouter with the existing key, and make a self-hosted endpoint a
first-class setting. Keep the feature off by default because of privacy. Reasons:

1. The blocking question in the item ("OpenRouter has no embeddings, so a second provider
   is required") is false. The remote path needs no new credential or integration.
2. The measured local cost is not prohibitive, but it is not free:
   - a forced base-image change from bookworm to trixie (+14 MB, and it affects everything in the image);
   - +32 MB binary;
   - +190–220 MiB of resident anonymous memory, up to about 580 MiB peak if batch size is not capped;
   - +12 % cold build, plus a new build-time CDN dependency;
   - an AVX2 floor for every self-hoster;
   - a model-download or cache lifecycle that must live on `/data`.
3. "Works offline and keeps the self-hosted promise" does not buy anything real: Tavern
   cannot generate without OpenRouter. The only real benefit of local is keeping the
   embedding content off third parties. The remote backend covers that when pointed at a self-hosted endpoint.
4. Storage should be a BLOB column with in-process brute-force cosine, keeping `model_id` and `dim` per row.
   sqlite-vec is not needed.

**If a local backend is wanted later:**
- Use fastembed with fp32 bge-small (not int8), ONNX batch ≤ 8 and 2 intra-op threads.
- Put the cache in `/data/models`, fetched on first enable. Do not bake it into the image.
- Build on trixie, and only in a `tavern-api:main-embed` variant image so the default image stays as it is.
- If avoiding trixie or AVX2 matters more than speed, use candle instead: it builds on bookworm, adds about 8 MB,
  and runs 2–4× slower.

**Where this differs from July.** July rejected Local as "the worst fit on the target box"
because of memory pressure (4 vCPU, 7.8 GiB, 3 GiB swapped, 92 % disk). The box
now has 12 vCPU, 16 GiB (8.3 GiB available) and 43 % disk used, so a local model *fits*.
It is still second choice, but now because of engineering and ops cost rather than headroom.
July also estimated the local build at +25–30 MB and baking the weights at +110–160 MB. Measured:
- the binary delta is +32 MB;
- July missed the bookworm → trixie requirement (+14 MB);
- baking adds +67 MB (int8) or +134 MB (fp32);
- the brute-force estimate was about 10× optimistic, though it is still fine.

## Caveats

- There are no real Zen 3 numbers. The amd64 timings are Rosetta-emulated. The next step, if Local is
  pursued, is a one-off run of the bench image on the deploy box, for example with `--cpus 2`, pulled and then removed.
- I did not measure RSS in a running `tavern-api`. The server needs a live auth service to
  boot. The bench's resident delta, added to the prod container's 209 MiB, is the estimate.
- Latency runs are p50 over 15 single calls or 5 batches, which is small. The Mac "all cores" runs were noisy.
- The image and build figures come from local OrbStack builds, not the Woodpecker agents.

## Cleanup

- Removed images: `tav41-spike:amd64-base`, `:amd64-trixie-base`,
  `:amd64-trixie-embed`, `:amd64-trixie-bench`, `:arm64-trixie-bench`,
  `:arm64-bookworm-candle`.
- Removed volumes: `tav41-models`, `tav41-models-amd64`.
- Left in place: the BuildKit cache, about 12.7 GB reclaimable and shared with other builds
  (`docker builder prune` if wanted), and the pulled `debian:trixie` base.
- The model caches and candle weights (about 330 MB) are in the session scratchpad.
