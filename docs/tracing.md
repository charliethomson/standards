# Tracing

## Rule

**Services export every span; the collector decides what to keep.** A span is one unit of
work that finishes, named for what it is, carrying the correlation ids as attributes, and
marked ERROR only when someone should look. A trace completes inside the collector's 60 s
decision window. Every request leaves exactly one completion log line, errors included.
Clients propagate `traceparent` and identify themselves with `x-<product>-*` headers; the
server adopts a client's trace id (and continues a service's trace), and echoes it so a
user-visible `Ref:` finds the trace.

This is the tracing layer on top of [observability.md](observability.md) (bootstrap, the
pipeline, privacy basics). Reading traces back is
[grafana-dashboards.md](grafana-dashboards.md).

## Why

Head sampling at 10 % hides 90 % of errors and makes every trace a coin toss. Process-lifetime
spans never end, so they never export, and a trace the collector judges before it finishes
is judged wrong. Per-line and per-item spans spend the whole budget on noise. A trace nobody
can find from a user's bug report is a trace nobody reads. Each rule below comes from one of
those failures in a real service.

## Sampling & retention

- **Services run `Sampling::AlwaysOn`** (liblog). They never head-sample in production:
  `AlwaysOn` even ignores a remote parent's `sampled=0`, because the decision belongs to the
  collector, which has the whole trace. `SAMPLE_RATE` / `OTEL_TRACES_SAMPLER` stay as
  overrides for local work or an emergency.
- **The collector tail-samples.** A trace is kept if any policy says so:

  | Policy | Keeps |
  |---|---|
  | errors | any span with status ERROR |
  | slow-http | an HTTP trace (`http.request.method` set) over **1 s** |
  | slow | any trace over **10 s** (background work) |
  | keep | any span with `sampling.keep = true` (code forcing it) |
  | baseline | **5 %** of everything else, so healthy traffic stays visible |

  A service whose normal unit is legitimately long (a streamed reply) gets a policy on a
  domain attribute (`time_to_first_token_ms >= N`) instead of total duration. Services not
  yet on this standard pass through untouched until they move, so their own head sampling
  doesn't compound with the baseline. Re-tune thresholds from the span metrics, not by feel.
- **Span metrics are computed before sampling.** RED per span name comes from 100 % of spans,
  so dashboards and alerts see all traffic while the trace store keeps ~5 % of healthy traces.
- **Retention:** traces 7 d; logs 7 d by default, a product's streams longer (e.g. 14 d) when
  there's a reason; metrics per the Prometheus config. A week answers "what happened on
  Tuesday"; tail sampling keeps a week small.

## Span shape

- **One span = one unit of work that finishes.** A request, a poll, an upload attempt, a
  cache refresh pass. If you can't say when it ends, it isn't a span.
- **A trace completes inside 60 s** (the collector's `decision_wait`). Work that can run longer
  is modelled as **short linked roots per phase** — `export.run.start`, per attempt, per
  reconnect, `export.run.end` — each a new root (`parent: None`) linked to the one before
  (`follows_from` / `add_link(current_span_context())`). The end span carries the total
  `duration_ms`.
- **Loops, supervisors, connections and streams never own a span.** Each iteration or event
  that is real work gets its own root. A `tick_loop` root, a connection-lifetime span or a
  span around a multi-hour job never exports, and parents everything under it into one
  unreadable trace.
- **Per-item work is DEBUG.** Found the hard way, each had to be demoted:
  - `libcmd` opened a span per line of child-process output;
  - an `ffmpeg.run` span covered the whole process lifetime;
  - a per-object metadata fetch inside a cache refresh turned one cold restart into a single
    ~11 000-span trace, ~94 % of that service's spans.

  Count items on the parent (`entries`, `bytes`, `attempts`) instead.
- **A span that waits for something that may never come is closed at 55 s** with
  `outcome = "timeout"` (first frame, first signal, a session that never starts), so it
  still lands inside the decision window.
- **An exported span accounts for its own duration.** Every awaited phase that can be slow
  gets an INFO child, and waits on shared resources are recorded as fields, so a slow span
  splits into queueing and work: `acquire_ms` (a pool connection), `lock_wait_ms`,
  `permit_wait_ms` (a semaphore), `send_wait_ms` (a full channel), `queue_wait_ms` (time on
  a queue before pickup). A driver with no spans of its own (sqlx) shows up only as an
  unexplained gap otherwise.
- **Boot is a short root.** Startup I/O (connect, migrate, first load) runs under one
  `<service>.startup` root (`parent: None`); work spawned during boot starts its own roots
  rather than collecting under the boot trace for minutes.
- **A connection or stream gets a connect span and one summary event at close.** The
  connect span covers the handshake/subscribe only; the close emits one INFO event
  (`sse.disconnected`, `ws.closed`) from a drop guard with `duration_ms`, counts
  (`events_sent`, `bytes_sent`, `lagged`) and the correlation ids **written out
  explicitly**, because a drop guard runs outside any span.
- **A cache hit is DEBUG; the miss is INFO.** The miss is the real round trip.
- **Crossing a boundary carries the context.** In-process (channel, actor, command queue):
  carry a `Span` only when the receiver's work is bounded by the caller's (a command the
  caller awaits); the receiver becomes its child and records `queue_wait_ms`. Otherwise
  carry a `SpanContext` and start a root linked to it. **Never store a `Span`, or a clone of
  one, in anything that outlives the unit**: a held clone keeps the span open and the trace
  unfinished. Across HTTP: `traceparent`. Across a custom RPC: an optional `traceparent` field
  on the envelope, outside any oneof, so old peers ignore it and new peers without one start
  a root.
- **Never hold `span.enter()` across `.await`**; use `.instrument(span)`. A spawned task either
  `.in_current_span()` (short, part of this work) or starts a new root (long-lived). Never
  `.instrument(Span::current())` on a long-lived spawn: it inherits a parent by accident and
  keeps it open.
- **Hot, cheap endpoints are DEBUG:** health, metrics, media segments, ingest. Their request
  span exists for local debugging and doesn't export. Exception: a route a *service* calls
  with an exported CLIENT span must export at the same level as that span, or the
  service-graph edge never pairs. Both sides follow one level rule; when the server can't
  tell from the route (one ingest route, many event kinds), the caller names the kind in a
  header and both sides key off it.

## Names

- `<component>.<unit>[.<phase>]`, lowercase dot.case, stable: `orders.sync`,
  `export.run.end`, `cache.refresh`. **No ids or user values in a name**; they're attributes.
  Always set the name explicitly (`#[instrument(name = "...")]`); the fn-name default changes
  on a rename.
- **HTTP server root:** span name `http.request`, `otel.name = "{METHOD} {route template}"`
  (`GET /api/orders/:id`; just `{METHOD}` when unmatched), `otel.kind = "server"`. Handler
  children: `api.<resource>.<op>` (`api.orders.create`).
- **Outbound calls: one CLIENT span per wire call**, opened with liblog's `client_span!`,
  named for the peer (`s3.get`, `db.query`, `openrouter.chat`, `payments.charge`) and
  carrying `peer.service` (or `db.system` + `db.namespace`) plus `server.address` and
  `server.port`. Grafana's service graph draws an edge only from a CLIENT span whose direct
  child is an exported SERVER span, or from one with those peer attributes; everything else
  exports as INTERNAL and draws nothing.
  - A span wrapping several calls (a transaction, a retry loop) is INTERNAL; each call
    inside it is its own CLIENT span.
  - If an SDK opens its own CLIENT span (an auth SDK's `auth.rpc.me`), your wrapper
    (`auth.me`) is INTERNAL. CLIENT under CLIENT draws an `unknown` node.
  - Calling another fleet service: `client_span!` + `inject_span` on the request, and the
    peer's `server_span!` becomes its child. `peer.service` is the peer's `service.name`.
  - An outbound HTTP span ends at the response headers. A body read that can be slow is a
    sibling span (`http.response.body`, `http.response.body.size`).
- **Log messages that a dashboard or alert matches are part of the contract.** Renaming one
  updates the dashboard and the alert in the same commit.

## Levels

| Level | Use | Exported by default |
|---|---|---|
| ERROR | the operation failed and someone should look; logged **once**, at the boundary that decides the outcome, and recorded on the span | yes |
| WARN | degraded but recovered (a late retry succeeded, a fallback was taken) | yes |
| INFO | spans for units of work; one completion event per unit | yes |
| DEBUG | sub-steps inside a unit; per-item events; hot endpoints | no |
| TRACE | per line / frame / tick | no |

**WARN means recovered.** A retry that may still fail is DEBUG (or a field on the span)
until the outcome is known; the give-up is the one ERROR. Something left broken until a
person intervenes is ERROR, not WARN.

**Expected outcomes are INFO or DEBUG, never WARN or ERROR:** not found, 4xx, a private or
missing upstream resource, end of stream, a declined credential. A WARN nobody acts on
trains everyone to ignore WARN.

The export filter defaults to `info` with noisy dependencies capped (liblog's
`NOISY_TARGETS`), overridable per layer from env without a rebuild. The list and the
override trap are in [observability.md](observability.md#filters).

## Errors

- **Marking and logging are separate duties.** A span that fails sets `otel.status_code =
  "ERROR"` and `error.type` where it fails, and logs nothing. The boundary that decides the
  outcome logs it, **once**. `#[instrument(err)]` on every layer logs the same failure once
  per layer.
- `error.type` is a stable kind (an error key, `http.502`, `timeout`), never the message: a
  message can carry row values or upstream content. One spelling everywhere, `error.type`,
  on the span and on the event.
- **For a request, the one log is the completion event.** A 5xx's error mapper records
  `error.type` and a sanitised detail onto the request span (through the request-scoped
  handle, see Attributes) and returns; the request layer's `http completed` line goes out at
  ERROR carrying them. A 4xx is not an error: the completion line stays INFO with its
  `error.type` so it can still be counted.
- **Clients get fixed text, logs get the detail.** A 5xx body says what kind of thing failed
  in words chosen for the client, never `err.to_string()` of a storage or upstream error
  (SQL, paths, upstream bodies). [error-handling.md](error-handling.md) has the mapper.
- **Library crates don't log the errors they return.** The caller decides the level. A
  vendored API client that WARNs on every non-2xx before returning `Err` produces two lines
  per failure; it records on its CLIENT span instead. Never `.ok()` / `let _ =` an error
  without logging it at the level the caller would choose.
- **A retried call records what the next failure needs:** `attempts`, `first_attempt_ms` and
  `first_error` (the `error.type`) on its span, and times the recovery (re-dial, handshake)
  as a child. An unexplained stall usually explains itself on the next occurrence.
- **Panics are logged.** In a request: put the panic catcher *inside* the request layer, so a
  panic is a 500 with a completion line and a trace id. Everywhere else: install a panic hook
  at startup that logs one ERROR (`error.type = "panic"`, the location, never the payload if
  it can hold user data), so a background task's panic isn't only on stderr.
- **A fatal error after telemetry is up is logged before `main` returns** (`error.type`,
  `"refusing to boot"`), then the guard is flushed. Returning `Err` from `main` reaches
  stderr only, which is `docker logs` only.
- A failed subprocess logs a short, sanitised stderr tail (the last few lines, URLs
  replaced with `<url>`) on its exit event, not the whole stream.
- Recording a `&dyn Error` field makes `tracing-opentelemetry` copy the whole source chain
  into `exception.message`. If any source can hold upstream content (a body, a URL), record
  `error.type` plus a sanitised message instead.

## Attributes

- **Correlation ids on spans and on events.** Every span doing per-entity work carries the id
  (`order_id`, …), and every log event inside it carries it as a field too. Span fields don't
  reach Loki, so a log line without the id can't be filtered to its entity.
- **One name per concept, codebase-wide.** `order_id`, never `order` in one place and `id` in
  another; if `attempt` and `run_id` both exist, each means exactly one thing. Adding a field
  whose name is already taken for a different meaning gets a new name (`layer_reason`, not a
  second `reason`).
- **`outcome` on every unit-of-work span**, from one vocabulary: `ok`, `failed`,
  `timeout`, `cancelled`, `not_found`, plus domain outcomes (`uploaded`, `skipped`,
  `requeued`) named once and reused. Never `success`/`error` in one place and `ok`/`failed`
  in another. `duration_ms` on any end span whose total isn't its own duration; durations
  are always `_ms`, never `_secs`.
- **Declare every field you fill later as `field::Empty`** when the span is created;
  `record()` of an undeclared field is silently dropped. Request-level fields learned deep
  in a handler (`enduser.id`, `error.type`) are recorded through a **request-scoped handle**
  to the request span (a task-local set by the request layer), not `Span::current()`, which
  by then is some child.
- **The completion event has a fixed field set.** Every `http completed` line carries the
  same keys, with empty values where they don't apply (`error.type = ""` on a 200), so the
  JSON file and the console always show them. In Loki, structured metadata **drops empty
  values and turns dots into underscores**: `error.type` is queried as `error_type`,
  `enduser.id` as `enduser_id`, and a 200 simply has no `error_type`
  (`| error_type != ""` finds the failures).
- **Fields you filter or aggregate on are flat.** A `Valuable` struct reaches Loki as one
  opaque blob; log the enum's kind as its own field (`result_kind`, `reason`) next to it.
- **Standard HTTP set (≤ 16):** `otel.kind`, `otel.name`, `otel.status_code`, `error.type`,
  `http.request.method`, `http.route`, `url.path`, `http.response.status_code`,
  `client.address`, `enduser.id`, `session.id`, `client.app`, `client.version`,
  `client.platform`, `client.install_id`, plus one auth attribute (`auth.method`).
- **`#[instrument(skip_all, fields(...))]` is the default.** Record chosen fields. Never
  `Debug` a whole struct (a request type with a token in it); give credential-carrying types
  a redacting `Debug` or none. Never `ret` on anything that can hold a URL or token.
- **Record numbers as `i64` (or `f64`), never `u64`/`usize` or `%`/Display.**
  tracing-opentelemetry exports unsigned integers (and anything formatted with `%`) as
  strings, on span fields and span events alike, so TraceQL can't compare them
  (`span.duration_ms > 1000` quietly matches nothing). Convert at the call site:
  `duration_ms = i64::try_from(d.as_millis()).unwrap_or(i64::MAX)`. Ids stay strings.
  (Log records through liblog's OTLP appender keep a `u64` as an integer, so Loki filters
  work either way; the rule is for Tempo, and following it everywhere is simplest.)
- **Cap attributes.** liblog sets `max_attributes_per_span(32)`; a span that hits it is
  carrying things that belong on child spans or events.
- **Security-relevant changes emit an audit event** (`audit = true`, `action`, `enduser.id`,
  the key changed); never the value.

## Privacy

Extends [observability.md](observability.md#privacy-non-negotiable). **Never on a span or a
log:**

- request/response bodies, query strings, tokens, credentials;
- **signed URLs**, including inside an error's `Display`. `reqwest::Error` prints its URL:
  log `err.without_url()`. Signed CDN URLs are capabilities;
- process **argv** that contains URLs or secrets (log the program and a count);
- free-text user input (search and filter terms);
- a reverse proxy's access/error log with the query string (strip `?…` in the proxy's log
  format).

IDs, counts, durations, route templates and status codes are fine.

## Client ↔ server contract

Clients don't export spans. (Usage events are separate: they go to the product server's
`POST <api base>/telemetry/events` and are stamped with the same identity, see
[client-analytics.md](client-analytics.md).) On every request whose headers they control,
they send:

| Header | Value |
|---|---|
| `traceparent` | `00-<32 hex trace>-<16 hex span>-01`, a **fresh trace per user action**. A 401 → refresh → retry keeps the trace id with a new span per attempt; a proactive/background refresh gets its own trace |
| `x-<product>-client` | stable app slug: `web`, `ios`, `macos`, `winui`, `cli`, … |
| `x-<product>-client-version` | the fleet version ([versioning.md](versioning.md)); web: the git short hash |
| `x-<product>-platform` | the OS actually running (`ios`, `macos`, `windows`, `linux`); a web client sends `browser` |
| `x-<product>-install-id` | UUIDv4 persisted per install; **never in a URL** |
| `x-<product>-session-id` | UUIDv4 per launch / page load |
| `User-Agent` (native) | `<Product>/<app>/<version> (<platform>; <os>)` |

- Ids come from a CSPRNG and are never all-zero. Header values are printable ASCII, ≤ 64
  chars; the server drops anything else.
- **Identity headers go only to the product's own origin.** Gate on the server origin before
  adding them. Third-party hosts (CDNs, avatars, platform APIs) never see them. Most HTTP
  stacks (browser fetch, URLSession, reqwest's default policy) re-send custom headers on a
  redirect, so the rule has two halves: **the server never redirects an identity-bearing
  request cross-host**, and a client that handles redirects itself drops `traceparent` and
  `x-<product>-*` on a cross-host hop the way it drops `Authorization`.
- **The server adopts a client's trace and continues a service's.** An app or browser never
  exports the span its `traceparent` names, so parenting on it leaves every client trace in
  Tempo as `<root span not yet received>`, with blank service and name columns, and root-based
  searches (`rootServiceName`, `rootName`) miss the request entirely. For those callers the
  request span keeps the inbound trace id with no parent:
  `liblog::adopt_client_trace(&span, &cx)`, which records `client.span_id` and links the
  client's span. A caller that is a fleet service exports its
  CLIENT span, so there the request span is its child (`liblog::server_span!`) and the
  service-graph edge forms. Extract and adopt before the span is entered, read, or has
  children; they capture the trace id at that moment.
- **The server** records the headers and `enduser.id` on the request span, and echoes the
  trace id in **`x-<product>-trace-id`** on every response that has one (its own, or the
  caller's when no span exporter is installed, as in local development). The header is listed in
  CORS `expose_headers`, and the outermost layer stamps it so error and panic responses get
  it too. JSON error bodies carry the same id as **`traceId`**.
- **Every request that can show a `Ref:` exports its completion line.** Tempo can't search
  by trace-id prefix and tail sampling drops most healthy traces, so a `Ref:` resolves
  through Loki's `trace_id` metadata on that line. A request path that skips the completion
  line on error can't be found from a bug report.
- **Clients show `Ref: <first 8 hex>`** on errors, with copy-the-full-id, and log the full id
  locally. A terminal client (a CLI) prints the full 32-hex id instead: the user copies text
  out of a terminal anyway, and the full id needs no prefix lookup. The trace id on an error is the header echo, then the body's `traceId`, then the id
  the client sent.
- **Loads that can't carry headers** (`<video>`, `<img>`, `EventSource`, `AVPlayer`, HLS
  segments) are traced through the signing/mint call that precedes them. A reconnecting
  stream client that can set headers sends a fresh `traceparent` per connection attempt.
  High-frequency sub-requests a client does control (HLS segments through a custom loader)
  send the identity headers but no `traceparent`: a trace per segment is noise.
- The identity headers go to the product's own origin and nowhere else, including the
  fleet's other services (the app registry): they are the product's identity, and another
  origin has no use for them.

## Pipeline hygiene

The collector is fleet-shared: one service at 100 % can flatten it for everyone. So:

- **`memory_limiter` first in every collector pipeline.** It refuses (senders retry) instead of
  being OOM-killed with everything in flight.
- **Pinned image tags** for the collector, Tempo, Loki and Grafana. Sampling and connector
  config keys churn between releases; bump deliberately.
- **Split the trace path:** `traces/in` (receive → `span_metrics` + `forward`) then `traces`
  (`forward` → `tail_sampling` → Tempo), so span metrics and sampling each see the full
  stream.
- **Scrape the pipeline's own metrics** (otelcol `:8888`, Tempo, Loki) **and alert on them.**
  At minimum: collector refused / failed-to-export spans or logs, and Tempo's
  `tempodb_blocklist_tenant_index_errors_total`. A saturated or broken pipeline looks exactly
  like a quiet one. Cautionary tale: a handful of torn (0-byte) Tempo blocks made every
  blocklist poll fail for three months. Retention and compaction saw an empty tenant, and
  ~256 k uncompacted blocks grew the volume to 40 GB. Nothing alerted, because nothing
  scraped Tempo. If the trace store only ever grows, check `tempodb_retention_deleted_total`.
- **Span-metrics label gotcha.** The `span_metrics` connector stamps `service.name` and
  `collector.instance.id` on every datapoint. Before a Prometheus exporter, delete both:
  `service.name` sanitises into a duplicate `service_name` label and the exporter fails the
  whole metric while the push still answers 200, and `collector.instance.id` is a fresh UUID
  per collector start, which begins a new set of series on every redeploy. Map
  `service.name` to a `service_name` datapoint label yourself.
- **Loki retention only runs with the compactor's `retention_enabled`.** Without it,
  `retention_period` is decoration.
- **Don't store logs twice.** A container that ships logs over OTLP is excluded from the
  stdout log shipper: it carries the `dev.thmsn.telemetry=otlp` compose label, and the
  shipper drops labelled containers ([observability.md](observability.md#the-pipeline)).
- **Size the batch exporter for always-on.** A service exporting every span needs room for a
  burst: `OTEL_BSP_MAX_QUEUE_SIZE=8192` (the default 2048 drops spans on a busy restart).
- **Know the trace store's limits.** Tempo caps a trace at 5 MB (a trace past it is
  truncated, usually one with a per-item span leak) and refuses a search window over
  168 h. Service-graph pairing loss shows as
  `otelcol_connector_servicegraph_expired_edges`.
- **Grafana links the three signals:** trace → logs (by trace id), trace → metrics (span
  name RED), and log → traces (by trace id and by the correlation id). Provisioning is in
  [grafana-dashboards.md](grafana-dashboards.md#trace--log--metric-links).

## Checklist

- [ ] Services init liblog with `Sampling::AlwaysOn`; no production head sampling.
- [ ] Collector tail-samples (errors / HTTP > 1 s / > 10 s / `sampling.keep` / 5 %); span
      metrics before sampling; traces 7 d, logs 7 d (longer per product when justified).
- [ ] No span outlives 60 s: long work is linked short roots per phase, end span carries
      `duration_ms`; unbounded waits closed at 55 s (`outcome=timeout`); no
      loop/supervisor/connection spans (connections: connect span + close summary event);
      per-item work and cache hits DEBUG; boot under `<service>.startup`.
- [ ] Slow spans account for their time: phases as children, `*_wait_ms` / `acquire_ms`.
- [ ] Context across queues: `Span` only when the receiver is bounded by the caller,
      otherwise a linked root; no stored `Span` clones.
- [ ] No `span.enter()` across `.await`; long-lived spawns start a new root.
- [ ] Span names explicit `<component>.<unit>[.<phase>]`, no ids; `http.request` with
      `otel.name = "{METHOD} {route}"`; handlers `api.<resource>.<op>`.
- [ ] One `client_span!` per wire call with `peer.service`/`db.system` + `server.address`;
      wrappers INTERNAL; fleet peers paired with `inject_span` / `server_span!`.
- [ ] Every request leaves one `http completed` line with a fixed field set, errors
      included; 5xx → ERROR + `error.type` recorded by the mapper, 4xx INFO; no second log.
- [ ] WARN only for recovered; retries DEBUG until the outcome; expected outcomes INFO/DEBUG.
- [ ] Failed spans marked where they fail, logged once at the boundary; libraries don't log
      returned errors; no silent `.ok()`; panic catcher inside the request layer and a
      panic hook; fatal boot errors logged before `main` returns.
- [ ] 5xx bodies carry fixed client text, never storage/upstream detail.
- [ ] Correlation ids on spans **and** events; one name per concept; `outcome` from the
      shared vocabulary; durations `_ms`; late fields declared `Empty`; filterable fields
      flat; `skip_all` default; numeric fields as `i64`/`f64`.
- [ ] No bodies, query strings, tokens, signed URLs (incl. error `Display` →
      `without_url()`), argv or free text on spans/logs.
- [ ] Clients send `traceparent` + `x-<product>-*` to the own origin only; server adopts
      client traces (`adopt_client_trace`) and continues service traces (`server_span!`);
      echoes `x-<product>-trace-id` (CORS-exposed) and `traceId` in error bodies; clients
      show `Ref:`.
- [ ] `OTEL_BSP_MAX_QUEUE_SIZE=8192` on always-on services.
- [ ] Collector: `memory_limiter` first, pinned images, self-metrics scraped + alerted
      (refused/failed export, Tempo blocklist errors); span-metrics labels cleaned.
- [ ] Grafana trace ↔ log ↔ metric links provisioned.
