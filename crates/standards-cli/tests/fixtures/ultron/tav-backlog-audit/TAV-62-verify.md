# TAV-62 verification: tavern tracing end to end (post HOMELAB-15)

Run 2026-09-30 ~12:32–12:40Z, read-only against Grafana (Viewer token from `~/git/homelab/mcp/.env`, GET
datasource-proxy + `POST /api/ds/query`). Nothing in tavern, homelab or vision state was changed.
Harmless probes sent to prod: 5 unauthenticated tavern GETs with a random `traceparent` and
`x-tavern-client: web` (12:32:51–56Z); 6 cut-short static-asset GETs with `?sig=`/`?token=` (3 tavern,
3 Grafana, 12:33:59Z and 12:35:41–45Z). No credentials, no data created, no LLM calls.

Sampling data is only from after the HOMELAB-15 sync: otelcol uptime 5299 s at 12:36:28Z, so the
collector (with the new config) started ~11:08:09Z. Window used: **11:10:00Z → 12:36:28Z (86 min)**.

Live config confirmed: homelab `5d92088` `monitoring/otelcol/config.yml` tail_sampling policies
errors / slow-http (not generation) / slow (not generation) / slow-ttft
(`time_to_first_token_ms >= 8000`) / tavern-4xx / keep / baseline 5 % / not-on-standard; Loki live
`/config` has `retention_stream` `2w` for `{service_name=~"dev\\.thmsn\\.tavern\\..*"}`; Tempo
`block_retention: 168h0m0s`; Tempo datasource `tracesToLogsV2` → loki,
`{service_name="${__span.tags["service.name"]}"} | trace_id="${__trace.traceId}"`, ±5 m.

Shorthand: `T=…/api/datasources/proxy/uid/tempo`, `L=…/uid/loki/loki/api/v1`,
`P=…/uid/prometheus/api/v1` on `https://grafana.dev.thmsn.dev`.

## Summary

| # | Check | Result |
|---|---|---|
| 1 | web and iOS error `Ref:` resolves in Tempo | **PASS (telemetry side)** — real web 500 and real iOS 503 since the sync are both kept and resolve (Tempo + logs); 4xx probes kept. Seeing the Ref *rendered in the UI* was not observed. |
| 2 | failed generation kept, ws.generate → generation → generation.reply → openrouter.chat, ERROR | **CAN'T RUN** — no failed generation since 11:08Z. Needs a user action (below). |
| 3 | slow-TTFT generation kept by the TTFT policy | **PASS** |
| 4 | baseline ~5 % | **PASS (approximate)** — ~4 % of healthy tavern traces stored; small window, see numbers |
| 5 | no duplicate tavern logs from promtail | **PASS** |
| 6 | dashboard Traces row populates | **PASS** |
| + | tavern included in tail sampling, not passed through | **PASS** |
| + | gateway `?sig=` redaction | **PASS** (re-proved with `?sig=test`) |

## 1. Web and iOS error Ref resolves — PASS (telemetry side)

Real client failures after the sync, both kept by `errors` and both resolving:

- **web**: `ab9734ffd896bbe012bdb0a76ad805a3`, 11:43:04Z, `GET /api/v1/chats` → 500,
  `STATUS_CODE_ERROR`, `client.app=web` `client.version=bf3ee90`, `error.type=500`. Loki
  `{service_name="dev.thmsn.tavern.api"} | trace_id="ab9734ffd896bbe012bdb0a76ad805a3"` → 2 lines
  (`request failed with an internal error`, `error_kind=storage`, `error_id=ab9734ff` — i.e. the
  8-char Ref the web client renders via `shortTraceId`).
- **iOS**: `e9bd207d8f46ef72647eaaa1ea202dce`, 11:12:41Z, `POST /api/session/refresh` → 503,
  `client.app=ios` `client.version=0.1.456+456`, `auth.refresh` `error.type=auth_unavailable`. Loki
  same query → 2 lines (`auth service unavailable during refresh; 503`).
- Query: `GET $T/api/search q={resource.service.name="dev.thmsn.tavern.api" && status = error}`,
  start 11:08Z → both ids, nothing else.

4xx probes (`curl -H "traceparent: 00-<rand32>-<rand16>-01" -H "x-tavern-client: web" …`): every
response echoed `x-tavern-trace-id` = the sent trace id, and all five were in Tempo after the 60 s
decision wait (`GET $T/api/v2/traces/<id>`), each a single `dev.thmsn.tavern.api` server span with
`client.app=web`:

| trace id | request | status |
|---|---|---|
| `da155b237bf9f5d788490556266d5e5c` | GET /api/v1/presets | 401 |
| `97d1d009f91268cb4f6130dd91a8bb52` | GET /api/v1/tav62-verify-nonexistent | 401 (auth runs before routing under /api/v1) |
| `95924ccfe6b096cfc58c399d4fe03320` | GET /api/v1/presets?sig=TAV62SECRETSIG | 401 |
| `940a9b442d90849ccbc4ff59a90879f3` | GET /api/v1/health | 401 |
| `377b0926b612fc0067419705784bfb58` | GET /api/tav62-verify-nope | 404 |

Also: the `?sig=` probe's span has `url.path=/api/v1/presets` and no query attribute; the secret
appears nowhere in the Tempo trace or in Loki (`sum(count_over_time({job=~".+"} |~ "TAV62SECRET|sig=test" [15m]))` → empty).

Traces with a client-side root show `<root span not yet received>` in Tempo search (clients send
`traceparent` but don't export spans); this is expected and doesn't stop resolution.

Grafana keeps the 32-char id: Tempo's API drops a leading zero (`f82015afd5…`, 31 chars) but
`POST /api/ds/query` (queryType traceId) returns `traceID=0f82015afd5979e719e3b0316a3ef0a1`, and Loki
matches only the padded form, so the tracesToLogs link works.

**Not observed:** the Ref rendered in the app. To close that fully: next time the web or iOS app
shows an inline error with `Ref: xxxxxxxx`, click/copy it (web copies the full id) and paste into
Grafana → Explore → Tempo (TraceQL/trace id). I'd look for the trace and its Logs link returning
the matching `error_id`.

## 2. Failed generation kept with ERROR — CAN'T RUN

No failed generation since the sync. Span metrics (7 d):
`sum by (span_name,status_code) (increase(traces_span_metrics_calls_total{service_name="dev.thmsn.tavern.api",span_name=~"ws.generate|generation|generation.reply|openrouter.chat|chat.auto_title"}[7d]))`
→ `generation`/`generation.reply` ERROR = 1 each (the 2026-09-28 trace `13f402097fdc8be4478594568da85680`,
kept by the old pass-through), everything since the sync UNSET.

What *is* proven: the `errors` policy keeps tavern traces (the two in check 1; collector counter
`otelcol_processor_tail_sampling_count_traces_sampled{policy="errors",sampled="true"}` = 2 since
start), and generation traces are exempt only from `slow`/`slow-http`, which come after `errors`
in evaluation — every policy is OR'd, so an ERROR generation is kept regardless. The shape is proven
live on a successful generation: `b8ed92bf727bb1dcf6c850955a088171` = `ws.generate` (1 link) →
`generation` → `generation.reply` → `prompt.assemble` + `openrouter.chat`.

**User action needed:** make one generation fail server-side (an `Upstream` or internal error —
`server/api/src/ws/generate.rs:273` `span_is_error`; a client-fault error is INFO and won't be
ERROR). Easiest: in a throwaway chat, point the model at an unavailable/invalid OpenRouter model id
(or one whose context is too small for the prompt) and send one message, so OpenRouter rejects it.
Tell me the time (or the `Ref:` shown). I'll look for
`{resource.service.name="dev.thmsn.tavern.api" && name="generation" && status=error}` after 11:08Z,
check the chain ws.generate → generation → generation.reply → openrouter.chat, ERROR on
generation/generation.reply (openrouter.chat may be UNSET if the error surfaces after the stream,
as on 09-28), and the Ref matching the trace id.

## 3. Slow-TTFT generation kept — PASS

- `GET $T/api/search q={resource.service.name="dev.thmsn.tavern.api" && span.time_to_first_token_ms > 8000}`,
  start 11:08Z → `b8ed92bf727bb1dcf6c850955a088171` (reply, `openrouter.chat`
  `time_to_first_token_ms=14027`, 20.7 s) and `0f82015afd5979e719e3b0316a3ef0a1` (title,
  `chat.auto_title` → `openrouter.chat`, TTFT 10249).
- `otelcol_processor_tail_sampling_count_traces_sampled{policy="slow-ttft",sampled="true"}` = 2.
- Neither trace has an ERROR or 4xx, and both are generation traces (exempt from slow / slow-http),
  so `slow-ttft` is what kept them (baseline is the only other candidate, at 5 %).
- Gap: a *fast*-TTFT generation being dropped has not happened live (every generation in the
  window was slow); HOMELAB-15's synthetic run covers it. Next time the user generates with a fast
  model, the ws.generate trace should normally be absent from Tempo (5 % chance it's kept by
  baseline).

## 4. Baseline ~5 % — PASS (approximate)

Window 11:10:00Z → 12:36:28Z (86 min).

- Tavern server spans produced (span metrics, computed before sampling):
  `sum(increase(traces_span_metrics_calls_total{service_name="dev.thmsn.tavern.api",span_kind="SPAN_KIND_SERVER"}[5188s]))`
  @12:36:28Z = **296.5** (cross-check: sum of 5 m `increase` steps = 311). `resets()` = 3 in the
  window (series expiry around the two tavern restarts at 11:17 and 11:52); `increase` handles resets.
- Stored in Tempo: `GET $T/api/search q={resource.service.name="dev.thmsn.tavern.api" && kind=server}`
  limit 1000, same window → **51 traces**, containing **85** tavern server spans. By reason:
  - 38 with a 401 → kept by `tavern-4xx` (33 real web 401-then-retry pairs in bursts at 11:12, 11:17,
    11:41, 11:52, plus my 4 probes); 1 with a 404 (my probe)
  - 2 with ERROR (check 1)
  - 1 generation with slow TTFT (check 3)
  - **9 healthy-only** (all 200/ws.auth, all < 1 s): `9781db22…`, `923ee61e…`, `bed98b84…`, `cef79838…`,
    `dbd29446…`, `45e32acf…`, `e73c18b8…`, `31172478…`, `bd9633f7…` → baseline
- Healthy traces produced ≈ unstored spans (296 − 85 = 211, all healthy since anything 4xx/ERROR is
  kept) + 9 stored healthy = ~220. **Stored share = 9 / 220 ≈ 4.1 %** (3.8 % with the 311 figure).
  Binomial expectation at 5 %: 11 ± 3.2; 95 % CI for 9/220 is roughly 2–8 %.
- Collector-wide corroboration (baseline evaluates every trace, all services):
  `sum by (policy,sampled) (otelcol_processor_tail_sampling_count_traces_sampled)` since start:
  baseline true 343 / (343 + 6368) = **5.1 %**.
- Before the change (prior audit): ~95 % stored (pass-through).

Enough traffic? Enough to show tavern is sampled at a few percent and not passed through, and the
figure is consistent with 5 %; not enough to pin it tighter than ±~3 points (only ~220 healthy
requests, mostly one user's session bursts). A 24 h re-run of the same two queries would tighten it.

Note: `tavern-4xx` dominates what's stored — 38 of 51 tavern traces in the window are the web
client's routine 401 → refresh → retry bounce. The narrowing HOMELAB-15 describes (drop 401 from
the rule, have tavern set `sampling.keep` on Ref-rendered 4xx) is still a live option if that's noise.

## 5. No duplicate tavern logs from promtail — PASS

- `sum(count_over_time({container="/tavern-api-1"}[2h]))` @now → empty.
- Last promtail line from `/tavern-api-1` in 7 d: 2026-09-29T19:13:57Z
  (`query_range {container="/tavern-api-1"}` limit 1 backward).
- OTLP stream alive: `sum by (service_name) (count_over_time({service_name=~"dev\\.thmsn\\.tavern\\..*"}[90m]))`
  → `dev.thmsn.tavern.api` 294.
- tavern containers promtail still ships (not OTLP exporters, expected): `/tavern-web-1` 19,
  `/tavern-watchtower-1` 29, `/tavern-proxy-1` 2 (90 m). Only other hits for `tavern-api` text are 2
  dockerd `sbJoin … ep=tavern-api-1` journal lines (the restarts), not app logs.

## 6. Dashboard Traces row populates — PASS

`GET /api/dashboards/uid/tavern-overview` (v7, updated 2026-09-29T19:14:13Z, default range
`now-6h`), `$service` = `dev.thmsn.tavern.api`. The three Traces-row panel queries over 11:08Z → now:

- Error traces `{ resource.service.name = "$service" && status = error }` → 2
  (`ab9734ff…`, `e9bd207d…`); via `POST /api/ds/query` as the panel runs it: 1 frame, 2 rows, no error.
- Slow requests `{ … && span.http.route != nil && kind = server && duration > 1s }` → 0 (no slow
  tavern HTTP request occurred; not a failure).
- Slow first token `{ … && span.time_to_first_token_ms > 8000 }` → 2 (`0f82015a…`, `b8ed92bf…`).

## + Tavern in tail sampling, not pass-through — PASS

Config: `not-on-standard` passes a trace only if no span matches `^dev\.thmsn\.(ttr|tavern)\.`.
Live: 9 of ~220 healthy tavern traces stored (check 4) vs ~95 % before; tavern traces that also
touch `dev.thmsn.auth.tcp` (`auth.rpc.me`) are sampled, not passed through (all such stored traces
in the window are 401/ERROR ones).

## + Gateway `?sig=` redaction — PASS

The gateway has no access log; only error/abort lines carry the request. A cut-short
`curl --limit-rate 1k --max-time 2 "https://grafana.dev.thmsn.dev/public/build/6540.acf453ee99146ad315f3.js?sig=test&token=TAV62SECRETTOK"`
×3 at 12:35:41–45Z → Loki `{container="/caddy"} |= "6540.acf453"`: 3 ×
`aborting with incomplete response`, `"uri":"/public/build/6540.acf453ee99146ad315f3.js?redacted"`;
`|~ "TAV62SECRET|sig=test"` over 15 m across all streams → 0. The same trick against a tavern asset
(`/assets/mount-Dm571-hG.js`, 450 KB) produced no gateway line (response fits in buffers), so the
proof is on the Grafana route; the filter is on the gateway's default logger so it covers tavern too.

## Left open

1. Check 2 (failed generation) — needs the user action in section 2.
2. Check 1 UI half — a user-visible `Ref:` pasted into Grafana (section 1).
3. Optional: re-run check 4 over 24 h for a tighter ratio; decide whether 401 refresh bounces
   should stay in `tavern-4xx`.
