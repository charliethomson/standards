# TAV backlog audit — batch B (TAV-41, TAV-43, TAV-51, TAV-62)

Audited 2026-09-30 against tavern `main` @ `e62c16d`, homelab `main` @ `c6188ae` (= origin/main per
`git ls-remote`), vision, and live Grafana (Viewer token, GET-only datasource-proxy queries). Nothing
was modified.

Heads-up: vision shows **TAV-41 and TAV-43 were moved to `in_progress` at 2026-09-30 10:44Z** by
another session, with uncommitted work in agent worktrees (not on main, so not counted below):
- `.claude/worktrees/agent-a5255f178e4b7c269` on branch `spike/tav-41-embeddings`: new
  `server/engine/src/embeddings.rs`, `server/engine/src/bin/`, engine/api Cargo.toml + lock changes.
- `.claude/worktrees/agent-a382f1bd5fd525f98`: `routes/messages.rs`, `routes/personas.rs`,
  `routes/mod.rs` (TAV-43 part 3 role enum, per the TAV-43 comment of 10:45Z).

---

## TAV-41 — Semantic retrieval (two-tier lore + archived-history recall)

**Verdict: NOT STARTED on main** (a decision spike exists; implementation is in progress off-main).

What exists: spike commits `849c3e4` and `2ba67fa` (2026-07-30) added
`reviews/spike-embeddings-2026-07-30.md` and a standalone `spikes/embeddings/` workspace (outside
`server/` members and the Docker context). Spike recommends: two-tier, keyword hits fill budget first,
OpenRouter `/v1/embeddings` or a self-hosted OpenAI-compatible endpoint, local ONNX deferred, off by
default, plain BLOB + brute force (no sqlite-vec). The dependency (prompt trace) shipped in `929c5c8`.

| Requirement | Status | Evidence |
|---|---|---|
| Embedding provider decision recorded in `PLAN.md` before implementation | missing | `PLAN.md:256` still reads "RAG / vector memory ... — future"; no embedding/semantic text in PLAN.md. The spike only *proposes* a §11 amendment (`reviews/spike-embeddings-2026-07-30.md` §11) and states "PLAN.md is untouched". |
| Keyword tier byte-identical to today | n/a (nothing changed) | `server/engine/src/prompt/lorebook.rs` last touched by `21cec8d`/`d67cec3`/`d8c0f1d`, none semantic. |
| Semantic top-k tier with threshold | missing | `grep -a` for embedding/cosine/similarity/vector/sqlite-vec across `server/**/*.rs,*.toml,*.sql` finds only "embedded image" features. |
| Recall over archived history | missing | same grep; no history-embedding code or migration. |
| Trace shows tier, score, threshold per semantic chunk | missing | `server/api/src/routes/trace.rs` has no tier/score fields. |
| Embed on write + background history embedding + backfill + re-embed on model change (model id stored) | missing | no schema/migration for vectors in `server/db/migrations/`. |
| Docker image size/startup impact measured and stated | missing (partial prep) | spike ships `spikes/embeddings/bench-vm101.sh` harness; its figures are labelled ESTIMATE / laptop-only; no measured image-size delta for a real build. |

---

## TAV-43 — API contract: 201 on create, uniform pagination, role enum (breaking)

**Verdict: NOT STARTED on main.** Vision: `in_progress`, now decomposed into children TAV-88 (document
200+201, clients accept both), TAV-89 (cursor pagination `{items, next_cursor}` + legacy shim),
TAV-90 (flip to 201), TAV-91 (remove compat) — all `todo`. Part 3 (role enum) is being worked in a
worktree (see heads-up).

| Requirement | Status | Evidence |
|---|---|---|
| Creates return `201` | missing | Every POST create in `api/openapi.json` documents only `200` (e.g. `POST /characters`, `/chats`, `/personas`, `/presets`, `/lorebooks`, `/lorebooks/{id}/entries`, `/assets`, `/import/*`); the only non-200 success is `POST /telemetry/events` → `202`. Handlers return `Result<Json<…>>` (e.g. `server/api/src/routes/characters.rs:170-174`); no `StatusCode::CREATED` anywhere in `server/api/src`. `git log -S`/`--grep` finds no 201 commit. |
| Swift service layer updated for `.created` | missing | follows from above (TAV-88/90 open). |
| Uniform pagination on all lists | missing | Only `GET /chats/{id}/messages` has `limit`/`offset`; `GET /characters`, `/chats`, `/personas`, `/presets`, `/lorebooks`, `/lorebooks/{id}/entries`, `/lorebooks/{id}/links`, `/members`, `/chats/{id}/members`, `/characters/{id}/images`, `/settings` take no pagination params and return bare arrays (spec analysis of `api/openapi.json`). |
| `role` is an enum | missing | `MessageDto.role: String` at `server/api/src/routes/messages.rs:41` (spec: `{"type":"string"}` with a doc comment); `TailTurnDto.role`/`TracedTurnDto.role: String` at `server/api/src/routes/trace.rs:87,115`; WS `DraftTurn.role: String` at `server/api/src/ws/draft.rs:101`. Note the issue text is stale: `MessageRequest` never existed as a REST DTO on current history (`git log -S 'struct MessageRequest'` empty; message creation is WS-only), and there are now two generated clients (web + iOS), not three. |
| Clients regenerated; drift check green | n/a until changes land | Drift gate exists: `server/api/src/openapi.rs:2525` `committed_openapi_json_matches_the_server`. |

---

## TAV-51 — Tracing overhaul (program parent)

**Verdict: PARTIAL — all tavern-side work is DONE; what remains is TAV-62 plus shared/homelab items
that TTR-53 closed without doing, which no open work item owns.**

Children (vision): TAV-52..61, 63..69, 71, 72 **done**; TAV-70 (metrics stack/exemplars)
**cancelled**; TAV-62 **backlog**. Commits on main per report §8: `f41a4e0`, `faa7ce4`, `3145ecd`,
`ab7592b`, `f9f9d43`, `a91672d`, `7e41119`, `cfaabe3`, `4dbc35b`, `d14f888`, `df3a167`, plus follow-ups
`58a1b85` (auth continuation), `f818dff` (download body stream), and standard adoption `f9d3e1f`,
`4b04a2d`, `b7fc53f` (`OTEL_TRACES_SAMPLER=always_on`), `690769e` (i64 numeric attrs).

Parent promises vs owners:

| Promise (parent description / report) | Status | Evidence / owner |
|---|---|---|
| traceparent in/out, HTTP span continues client traces | done | `ab7592b`; Tempo: 233 web + 47 iOS traces carry `span.client.app` (7d) |
| WS frame spans linked to upgrade; generation not orphan | done | `a91672d`; Tempo trace `13f40209…`: `ws.generate`(link=1) → `generation` → `generation.reply` → `openrouter.chat` |
| LLM call span | done | `f9f9d43`; `server/openrouter/src/lib.rs:303` |
| ERROR status on server faults | done | same trace: `generation`/`generation.reply` STATUS_CODE_ERROR, `error.type=upstream` |
| attribute cap / sqlx event flood | done | `3145ecd` (liblog TTR-49), `faa7ce4` |
| content leaks (upstream bodies, reqwest URLs, serde, `?sig=` in **tavern** Caddy) | done | `f41a4e0` |
| identity attrs (`enduser.id`, `client.*`, `session.id`, conn id, outcome) | done | Tempo `span.session.id != nil` / `span.client.app != nil` return results; `client.app` values seen: web, ios, cli, macos |
| `X-Tavern-Trace-Id` echo + `Ref:` in clients | done | `server/api/src/middleware.rs:9`, `server/api/src/error.rs:43`; `apps/web/src/components/TraceRef.tsx`; `d14f888` iOS |
| Metrics crate / histograms / exemplars (G3) | deliberately dropped | TAV-70 cancelled |
| **Tavern included in otelcol tail sampling** (not pass-through) | **not done, unowned** | `~/git/homelab/monitoring/otelcol/config.yml` `not-ttr` policy still passes every non-`^dev\.thmsn\.ttr\.` service; requested in TTR-53 comments (05:24Z and 18:59Z 09-27) but TTR-53 closed 09-28 without it. TAV-62 only *verifies*; no open item implements it. |
| **TTFT policy** (`time_to_first_token_ms >= 8000`) and exempting tavern from generic >10s | **not done, unowned** | no such policy in otelcol config; same TTR-53 comment. |
| **Loki 14d for tavern logs** | **not done, unowned** | `monitoring/loki/config/loki-config.yml:32-35` retention_stream is TTR-only; live Loki `/config`: `retention_period: 1w`, one `2w` stream for `dev\.thmsn\.ttr\..*`. |
| **Gateway Caddy `?sig=` query redaction** (X5 gateway half) | **not done, unowned** | `~/git/homelab/gateway/Caddyfile` has no `log`/`request>uri` filter (grep empty; last commits `1cd20f6`, `5421222`, `09ce2ae`); asked in TTR-53 comment #1 item 6; vision search ("caddy", "sig") finds no open item for it. |
| Tempo 7d, tracesToLogsV2, promtail drop | done | see TAV-62 below |
| Re-verify end to end | open | TAV-62 |

Recommendation: file a homelab/TTR follow-up (or extend TAV-62's scope) for the four unowned
shared items above; TAV-62 cannot pass (3)/(4) until the first two land.

---

## TAV-62 — Re-verify tavern end to end once tail sampling and retention land

**Verdict: NOT STARTED, and still BLOCKED in substance** — TTR-53 is `done` (closed
2026-09-28T06:34Z) but it shipped tail sampling **scoped to TTR only**; tavern is passed through
untouched, so the sampling-dependent checks cannot be meaningfully run yet.

### (a) Preconditions

| Precondition | Live? | Evidence |
|---|---|---|
| TTR-53 done | yes | vision `TTR-53 done`, blocks TAV-62 |
| otelcol tail sampling exists | yes | homelab `941d494`; `otelcol/config.yml` `tail_sampling` (decision_wait 60s, errors / slow-http / slow / keep / baseline 5% / not-ttr); live metric `otelcol_processor_tail_sampling_count_traces_sampled` 24h: not-ttr sampled=true 54.2k, baseline 4.5k, slow-http 661, slow 216, errors 101 |
| **tavern (`dev.thmsn.tavern.api`) included in tail sampling** | **NO** | `not-ttr` OTTL policy `not IsMatch(service.name, "^dev\\.thmsn\\.ttr\\.")` keeps every tavern trace. Live: tavern spanmetrics server spans last 6h = 81.7, Tempo-stored tavern traces last 6h = 77 (~95%+, i.e. pass-through, not 5%). |
| **TTFT numeric policy** | **NO** | no `numeric_attribute` policy in config. Note the attribute is now `time_to_first_token_ms` (`server/openrouter/src/lib.rs:303`, renamed in the standard adoption), not `tavern.gen_ai.ttft_ms` as TAV-62's text says; Tempo: `span.tavern.gen_ai.ttft_ms` → 0 hits, `span.time_to_first_token_ms >= 0` → 14 traces. The generic `slow` >10s policy would also apply to tavern once included (needs the exemption requested in TTR-53). |
| Tempo 7d retention | yes | `monitoring/tempo/tempo.yml:38` `block_retention: 168h`; live `/status/config` backend_scheduler compaction `block_retention: 168h0m0s` (a `backend_worker` block shows the 336h default; the scheduler provider value is the configured one) |
| Loki 14d for tavern | **NO** | tavern stays at the 1w default (see TAV-51 table) |
| Grafana tracesToLogsV2 | yes | live Tempo datasource jsonData: `tracesToLogsV2` → loki, `{service_name="${__span.tags["service.name"]}"} \| trace_id="${__trace.traceId}"`, ±5m; plus tracesToMetrics, nodeGraph. Loki derived fields `trace_id` and `job_id` → Tempo present. |
| promtail drop of `dev.thmsn.telemetry=otlp` | yes | homelab `681e8f8` (2026-09-29T14:59-04:00) `promtail/config.yml:19-21`; tavern label at `deploy/compose/prod.compose.yml:27`. Loki: last promtail line from `/tavern-api-1` is 2026-09-29T19:13:57Z, none since. |
| Amended standard (TTR-47) followed | appears done on tavern side | `6e013ff`, `f9d3e1f`, `4b04a2d`, `b7fc53f`, `690769e`, `2c9fa98`, `e62c16d` (TTR-47 itself is still `backlog` as a program). |

### (b) Which checks can be run read-only now

| # | Check | Runnable read-only now? | Result of what was run |
|---|---|---|---|
| 1 | web and iOS error Ref resolves in Tempo | partially: can confirm client-originated traces land in Tempo, but a fresh error must be produced by a user (not read-only) | 7d: 233 traces with `client.app="web"` (32 with `http.response.status_code >= 400`), 47 with `client.app="ios"` (0 with ≥400, 0 errors). So a web 4xx Ref would resolve **today**, but only because tavern is pass-through; once tavern is tail-sampled, 4xx (not ERROR) traces keep at ~5% only, so most client-visible Refs for 4xx would not resolve. Worth a decision. |
| 2 | failed generation kept, shape ws.generate → generation → generation.reply → openrouter.chat with ERROR | yes (historical) | Trace `13f402097fdc8be4478594568da85680` (2026-09-28T00:35:48Z): shape matches; ERROR on `generation.reply` and `generation` (`error.type=upstream`), `ws.generate` has 1 link. `openrouter.chat` is **not** ERROR (`stream_finish=done`) — the upstream error surfaced after the stream. Kept by pass-through, so it does not prove the `errors` policy works for tavern. |
| 3 | slow-TTFT generation kept by TTFT policy | no — policy doesn't exist | 5 traces with `time_to_first_token_ms >= 8000` in 7d exist (09-28 00:18–00:35Z), all kept by pass-through. |
| 4 | baseline ~5% | yes, and it **fails** | ~95%+ of tavern traces stored (spanmetrics 81.7 server spans vs 77 Tempo traces in 6h). Tail-sampling metrics have no per-service label, so this is the best read-only proxy. |
| 5 | no duplicate tavern logs from promtail | yes, **passes** | no `/tavern-api-1` promtail lines after 2026-09-29T19:13:57Z; OTLP stream `{service_name="dev.thmsn.tavern.api"}` continues (116 lines in the last 24h). Other tavern containers (web, proxy, docs, watchtower) are still shipped by promtail as expected (not OTLP exporters). |
| 6 | dashboard Traces row populates | yes, **passes** (7d range) | live `tavern-overview` (provisioned, v7, updated 2026-09-29T19:14Z) has the Traces row; its three TraceQL queries over 7d return 5 error traces, 1 slow request, 5 slow-TTFT traces. Will thin out once tavern is sampled. |
| + | tavern in tail sampling rather than passed through | yes, **fails** | see (a). |

Also note: generation traffic in Tempo for the last 7d is only 2026-09-28 00:18–00:35Z (11 `ws.generate`),
so checks 2/3 rest on that one session.
