# Client analytics

## Rule

**Clients record product events and send them in batches to their own product server; the
server validates them, stamps the caller's identity and re-emits each one as an OTLP log
record; the collector routes those records to ClickHouse, and Grafana reads them back.**
No client talks to an analytics service directly, and no third-party analytics SDK ships in
any client. Events are enums, counts, durations and booleans, never user content.

This sits beside [tracing.md](tracing.md): traces answer "what happened to this request",
events answer "how is the app used, on which versions, and how does it feel". They share the
client identity contract ([tracing.md](tracing.md#client--server-contract)) and the
pipeline ([observability.md](observability.md#the-pipeline)). Reading events back is
[grafana-dashboards.md](grafana-dashboards.md).

## Why

- **Self-hosted everything** ([overview.md](overview.md#operating-context-read-this-first)).
  Google Analytics, Firebase, Mixpanel and the like are out. A self-hosted PostHog brings
  ClickHouse, Kafka, Redis and Postgres (≈ 16 GB) to serve one user.
- **The pieces already exist.** Every client already sends `x-<product>-client`,
  `-client-version`, `-platform`, `-install-id` and `-session-id`; every server already
  exports OTLP to a collector the homelab runs. An event is those five values plus a name
  and a few properties.
- **Relaying through the product server** keeps the identity-headers-to-own-origin rule,
  reuses the product's auth, and needs no new public ingest endpoint. The collector stays
  internal.
- **ClickHouse, not Loki.** "Distinct installs per version per day over 90 days" and
  funnels (`windowFunnel`) are what analytics queries are made of. Loki does neither well,
  and its 7 d retention is the wrong horizon for usage trends.
- **One path for everything client-side.** Web vitals, hangs and client errors are events
  too, so there's one endpoint, one schema and one store. (Grafana Faro was considered for
  web RUM: it needs its own browser-reachable ingest and covers only the web client.)

## The event

An event is one thing that happened in a client, named `<domain>.<verb_or_noun>`:

| Field | Type | Rule |
|---|---|---|
| `name` | string | `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){1,3}$`, ≤ 64 chars. Stable: renaming one is a dashboard change in the same commit |
| `ts` | RFC 3339 UTC | client clock when it happened |
| `seq` | int | per-session counter from 0; orders events and exposes gaps |
| `props` | object | ≤ 16 keys; key `^[a-z][a-z0-9_]*$` ≤ 32 chars; value a string ≤ 128 chars, a finite number or a bool. **Flat**: no arrays, no nesting, no null |

Identity is **not** in the event. The server takes it from the request (the `x-<product>-*`
headers and the authenticated user), so a client can't mislabel an event and the batch
doesn't repeat it 100 times.

### Reserved names

Every client emits these, so the fleet dashboard works for every product without edits.
Product events use the product's own domains (`job.create`, `recording.play`,
`chat.fork`); they never use a reserved prefix.

| Event | When | Props |
|---|---|---|
| `app.launch` | process start (web: page load) | `cold` (bool), `launch_ms` (to first interactive frame) |
| `app.foreground` | app becomes active (web: `visibilitychange` → visible) | |
| `app.background` | app resigns active (web: → hidden) | `foreground_ms` since the last foreground |
| `screen.view` | a screen / route becomes visible | `screen`: the **route template or screen id** (`library`, `/jobs/:id`), never a title or a concrete id |
| `error.client` | an error shown to the user, or an unhandled exception | `error_type` (stable kind), `handled` (bool), `screen`, `trace_id` when the error has a `Ref:` |
| `perf.web_vital` | web only, from the `web-vitals` library | `metric` (`LCP`, `INP`, `CLS`, `FCP`, `TTFB`), `value`, `rating` |
| `perf.hang` | a main-thread stall ≥ 250 ms (MetricKit on Apple; a UI-thread watchdog on WinUI/gpui) | `duration_ms`, `screen` |
| `perf.launch` | Apple only, from MetricKit's daily launch histogram | `p50_ms`, `p90_ms`, `count` |
| `cli.command` | CLI only, once per invocation | `command` (subcommand path, `job.add`), `outcome`, `duration_ms` |

Sessions need no events of their own: `session.id` is per launch / page load, so a session
is every event sharing it, and its length is the spread of their `ts`.

## Privacy (non-negotiable)

Extends [observability.md](observability.md#privacy-non-negotiable) and
[tracing.md](tracing.md#privacy).

- **Props are enums, counts, durations and booleans.** Never free text, titles, names,
  search terms, URLs, file paths, message content or error messages. An `error.client` carries
  `error_type`, not `error.message`.
- **Entity ids only when a funnel needs them.** `screen` is the route template; a
  specific id goes in a prop only if a query joins on it, and never a public short id that
  appears in a shareable URL.
- **No device fingerprinting.** `install_id` is the only per-device value: no IDFA/IDFV,
  hardware ids, screen resolution, locale or timezone beyond what `platform` already says.
- **The server doesn't attach `client.address`** to events, and the ingest route is
  excluded from any reverse-proxy access log that keeps the client IP.
- **No third-party analytics or crash SDKs in any client.** MetricKit and the `web-vitals`
  library are fine: they measure locally and report nowhere on their own.

## Client behaviour

Every client has one **event recorder** in its shared layer (web `src/telemetry/`, Apple
`<Product>Kit`, the WinUI shared core, the gpui/CLI crate). Views call
`record(name, props)`; nothing else knows about batching or transport.

- **Buffer and batch.** Flush when 20 events are queued, every 30 s while foregrounded,
  and on background / `pagehide` / window close / process exit.
- **Persist on native.** The queue lives in a JSON-lines file under the app's support
  directory (Apple: Application Support; WinUI: `ApplicationData.Current.LocalFolder` when
  packaged, else `%LOCALAPPDATA%\<Product>`; Rust: `libpath`'s data root), capped at
  **1 000 events, oldest dropped**. Web keeps it in memory; a lost tab is lost events.
- **Send only when signed in.** The endpoint is authenticated. Events recorded before
  sign-in stay queued and go with the first authenticated flush; a queue that never gets
  there ages out.
- **Retry on network errors and 5xx** with backoff (30 s doubling to 10 min). **Drop the
  batch on any 4xx**: a malformed batch will be malformed next time too. Drop events older
  than 7 days before sending.
- **Web flushes on `pagehide` with `fetch(..., { keepalive: true })`.** `sendBeacon` can't
  set the identity or auth headers. Keep a keepalive batch under 60 KiB (the browser cap is
  64 KiB for all in-flight keepalive requests).
- **iOS flushes inside `beginBackgroundTask`** when the scene goes to the background, so
  the request isn't frozen mid-flight.
- **The CLI flushes synchronously at exit** with a 1 s timeout and doesn't persist: a
  failed flush is lost, never retried on the next run.
- **Recording never fails the caller.** `record` is fire-and-forget, never throws, and
  never blocks the UI thread. A full queue drops silently. Debug builds log each event
  locally as well.
- **Hot-path volume is capped at the source.** No event per keystroke, scroll or frame.
  A thing that happens more than about once a second is a count on a later event, not an
  event.

## Server: `POST /api/telemetry/events`

Every full-stack product serves it, in its OpenAPI contract (so the generated clients have
it) and behind the product's normal auth.

```jsonc
// request
{ "events": [
  { "name": "screen.view", "ts": "2026-09-28T14:03:11.402Z", "seq": 7,
    "props": { "screen": "/jobs/:id" } }
] }
// 202 response
{ "accepted": 1, "dropped": 0 }
```

- **Whole-batch limits → 400:** body > 64 KiB, > 100 events, or not JSON of this shape.
  Clients drop a 400.
- **Per-event validation → dropped, never an error.** A bad name, a prop that breaks the
  rules above, a `ts` more than 7 days old or more than 5 minutes in the future: that
  event is dropped and counted, the rest are accepted. The response is 202 either way, so
  one bad event can't make a client retry a batch forever.
- **Emit one INFO log record per accepted event**, on the dedicated target
  `client_event`, with exactly these attributes:

  | Attribute | From |
  |---|---|
  | `event.source` | the literal `client` (the collector routes on it) |
  | `event.name` | `name` |
  | `event.props` | `props`, re-serialised as compact JSON |
  | `event.client_ts` | `ts` |
  | `event.seq` | `seq` (`i64`) |
  | `client.app`, `client.version`, `client.platform`, `client.install_id`, `session.id` | the validated identity headers, same as the request span |
  | `enduser.id` | the authenticated user |

  The log body is the event name. The record's resource is the server
  (`service.name = dev.thmsn.<product>.<service>`), which is how events are grouped by
  product. `event.props` is one JSON string because `tracing` field names are static; the
  ClickHouse view unpacks it.
- **The route is a hot endpoint** ([tracing.md](tracing.md#span-shape)): its request span
  is DEBUG and doesn't export. A dropped event is counted, not logged:
  `<product>_client_events_dropped_total{reason}` with `reason` one of `name`, `props`,
  `ts`, `batch`. Accepted volume is read from ClickHouse, not duplicated into Prometheus.
- **Kill switch:** `<PRODUCT>_CLIENT_EVENTS=false` makes the handler answer 202 with
  everything counted as dropped (`reason="disabled"`), so clients drain their queues instead
  of retrying.
- **`LIBLOG_LOG_FILTER` must pass `client_event=info`.** It does by default; a filter that
  narrows the log bridge keeps that target.

A sketch of the emit, inside the handler once identity and the event are validated:

```rust
tracing::info!(
    target: "client_event",
    event.source = "client",
    event.name = %event.name,
    event.props = %props_json,
    event.client_ts = %event.ts,
    event.seq = event.seq,
    client.app = identity.app.as_deref(),
    client.version = identity.version.as_deref(),
    client.platform = identity.platform.as_deref(),
    client.install_id = identity.install_id.as_deref(),
    session.id = identity.session_id.as_deref(),
    enduser.id = %user.id,
    "{}", event.name,
);
```

The handler is the same in every product. Copy it until the second product adopts it, then
move it into a shared `lib*` crate ([lib-ecosystem.md](lib-ecosystem.md)).

## Pipeline & storage

```
client ──batch──► product server ──OTLP logs──► otelcol ─┬─ event.source == "client" ──► ClickHouse (otel_logs)
                  (validate, stamp identity)             └─ everything else ──────────► Loki
                                                                          Grafana ◄── ClickHouse datasource
```

- **The collector routes, it doesn't copy.** A routing connector on the logs pipeline sends
  `attributes["event.source"] == "client"` to the ClickHouse exporter and everything else
  to Loki. Client events never land in Loki ("don't store logs twice",
  [tracing.md](tracing.md#pipeline-hygiene)). `memory_limiter` stays first.
- **ClickHouse keeps events 400 days** (a TTL on the table), long enough for
  year-over-year. It's small: a busy day of one user across every app is thousands of rows.
- **Dashboards query the `client_events` view, never `otel_logs` directly.** The homelab
  defines it over the exporter's table, so a change to the exporter's schema is one view
  edit:

  | Column | Source |
  |---|---|
  | `ts` | `event.client_ts` (client clock) |
  | `received_at` | the log record's timestamp (server clock) |
  | `product` | `service.name` minus the service suffix |
  | `service` | `service.name` |
  | `name` | `event.name` |
  | `props` | `event.props`, a JSON string (read with `JSONExtract*`) |
  | `app`, `version`, `platform`, `install_id`, `session_id`, `user_id` | the identity attributes |
  | `seq` | `event.seq` |
  | `environment` | `deployment.environment.name` |

- **The pipeline alerts on itself** like the trace path does: ClickHouse exporter send
  failures and `<product>_client_events_dropped_total` climbing (a client shipping a bad
  schema).

## Reading it back

A fleet **Clients** dashboard in the homelab monitoring stack covers every product through
the reserved events, with a `product` variable. At minimum: daily active installs by app,
versions in the wild (installs per `version` over the last 7 d), sessions and session
length, top screens, `error.client` by `error_type` and version (with the `trace_id` linking
to Tempo), web vitals p75 by metric, hangs by screen. Product dashboards
(`<product>-overview`) add panels for their own events. Funnels are ClickHouse
`windowFunnel` queries over `client_events`, kept as panels, not ad-hoc.

## Checklist

- [ ] No third-party analytics or crash SDK in any client; events leave only for the
      product's own server.
- [ ] One event recorder per client in its shared layer; views call `record(name, props)`.
- [ ] Every client emits the reserved events (`app.*`, `screen.view`, `error.client`, the
      `perf.*` that apply; `cli.command` for a CLI).
- [ ] Event names `<domain>.<name>`, stable; props flat enums/counts/durations/bools, no
      free text, URLs, titles or messages; `screen` is a route template.
- [ ] Clients batch (20 events / 30 s / on background), persist on native (≤ 1 000,
      oldest dropped), retry 5xx and network errors with backoff, drop on 4xx, send only
      when signed in; web flushes with `fetch` `keepalive`.
- [ ] Server serves `POST /api/telemetry/events` in the contract, behind auth: batch
      limits → 400, bad events dropped and counted, 202 otherwise; DEBUG request span.
- [ ] One `client_event` INFO log per event with `event.source = "client"` and exactly the
      attribute set above; no `client.address`; `<PRODUCT>_CLIENT_EVENTS` kill switch.
- [ ] Collector routes client events to ClickHouse only; `client_events` view; 400 d TTL;
      exporter failures and dropped events alerted.
- [ ] Product dashboard panels for product events query `client_events`.
