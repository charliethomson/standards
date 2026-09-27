# Tracing

## Rule

**Services export every span; the collector decides what to keep.** A span is one unit of
work that finishes, named for what it is, carrying the correlation ids as attributes, and
marked ERROR only when someone should look. A trace completes inside the collector's 60 s
decision window. Clients propagate `traceparent` and identify themselves with
`x-<product>-*` headers; the server echoes the trace id so a user-visible `Ref:` finds the
trace.

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
- **Crossing a boundary carries the context.** In-process (channel, actor, command queue):
  capture the caller's `Span` / `SpanContext` on the message and parent or link to it on the
  other side. Across HTTP: `traceparent`. Across a custom RPC: an optional `traceparent` field
  on the envelope, outside any oneof, so old peers ignore it and new peers without one start
  a root.
- **Never hold `span.enter()` across `.await`**; use `.instrument(span)`. A spawned task either
  `.in_current_span()` (short, part of this work) or starts a new root (long-lived). Never
  `.instrument(Span::current())` on a long-lived spawn: it inherits a parent by accident and
  keeps it open.
- **Hot, cheap endpoints are DEBUG:** health, metrics, media segments, ingest. Their request
  span exists for local debugging and doesn't export.

## Names

- `<component>.<unit>[.<phase>]`, lowercase dot.case, stable: `orders.sync`,
  `export.run.end`, `cache.refresh`. **No ids or user values in a name**; they're attributes.
  Always set the name explicitly (`#[instrument(name = "...")]`); the fn-name default changes
  on a rename.
- **HTTP server root:** span name `http.request`, `otel.name = "{METHOD} {route template}"`
  (`GET /api/orders/:id`; just `{METHOD}` when unmatched), `otel.kind = "server"`. Handler
  children: `api.<resource>.<op>` (`api.orders.create`).
- **Outbound calls:** `otel.kind = "client"`, named for the peer: `auth.me`, `s3.get`,
  `db.<op>`, `payments.charge`.
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

**Expected outcomes are INFO or DEBUG, never WARN or ERROR:** not found, 4xx, a private or
missing upstream resource, end of stream, a declined credential. A WARN nobody acts on
trains everyone to ignore WARN.

The export filter defaults to `info` with noisy dependencies capped (liblog's
`NOISY_TARGETS`: `h2`, `hyper`, `tonic`, `tower`, `reqwest`, `rustls`, `sqlx`,
`opentelemetry*`), overridable per layer from env without a rebuild
([observability.md](observability.md#filters)).

## Errors

- A failed span sets `otel.status_code = "ERROR"` and `error.type` (a stable kind, not the
  message). A 5xx response is an error; **a 4xx is not**.
- **One error log per failure.** Inner layers return errors; the boundary that decides the
  outcome logs and marks the span. `#[instrument(err)]` on every layer logs the same failure
  once per layer.
- A panic is a logged 500: put the panic catcher *inside* the request-logging layer.
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
- **`outcome` on every unit-of-work span** (`ok`, `not_found`, `uploaded`, `failed`, …).
  `duration_ms` on any end span whose total isn't its own duration.
- **Standard HTTP set (≤ 16):** `otel.kind`, `otel.name`, `otel.status_code`, `error.type`,
  `http.request.method`, `http.route`, `url.path`, `http.response.status_code`,
  `client.address`, `enduser.id`, `session.id`, `client.app`, `client.version`,
  `client.platform`, `client.install_id`, plus one auth attribute (`auth.method`).
- **`#[instrument(skip_all, fields(...))]` is the default.** Record chosen fields. Never
  `Debug` a whole struct (a request type with a token in it); give credential-carrying types
  a redacting `Debug` or none. Never `ret` on anything that can hold a URL or token.
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

Clients don't export spans. On every request whose headers they control, they send:

| Header | Value |
|---|---|
| `traceparent` | `00-<32 hex trace>-<16 hex span>-01`, a **fresh trace per user action**. A 401 → refresh → retry keeps the trace id with a new span per attempt; a proactive/background refresh gets its own trace |
| `x-<product>-client` | stable app slug: `web`, `ios`, `macos`, `winui`, `cli`, … |
| `x-<product>-client-version` | the fleet version ([versioning.md](versioning.md)); web: the git short hash |
| `x-<product>-platform` | the OS actually running (`ios`, `macos`, `windows`, `linux`) |
| `x-<product>-install-id` | UUIDv4 persisted per install; **never in a URL** |
| `x-<product>-session-id` | UUIDv4 per launch / page load |
| `User-Agent` (native) | `<Product>/<app>/<version> (<platform>; <os>)` |

- Ids come from a CSPRNG and are never all-zero. Header values are printable ASCII, ≤ 64
  chars; the server drops anything else.
- **Identity headers go only to the product's own origin.** Gate on the server origin before
  adding them. A cross-host redirect drops `traceparent` and `x-<product>-*` the way it drops
  `Authorization`. Third-party hosts (CDNs, avatars, platform APIs) never see them.
- **The server** continues the inbound trace, records the headers and `enduser.id` on the
  request span, and echoes the trace id in **`x-<product>-trace-id`** on every response that
  has one (its own id, or the caller's when nothing is exported). The header is listed in
  CORS `expose_headers`, and the outermost layer stamps it so error and panic responses get
  it too. JSON error bodies carry the same id as **`traceId`**.
- **Clients show `Ref: <first 8 hex>`** on errors, with copy-the-full-id, and log the full id
  locally. The trace id on an error is the header echo, then the body's `traceId`, then the id
  the client sent.
- **Loads that can't carry headers** (`<video>`, `<img>`, `EventSource`, `AVPlayer`, HLS
  segments) are traced through the signing/mint call that precedes them. A reconnecting
  stream client that can set headers sends a fresh `traceparent` per connection attempt.

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
  stdout log shipper.
- **Grafana links the three signals:** trace → logs (by trace id), trace → metrics (span
  name RED), and log → traces (by trace id and by the correlation id). Provisioning is in
  [grafana-dashboards.md](grafana-dashboards.md#trace--log--metric-links).

## Checklist

- [ ] Services init liblog with `Sampling::AlwaysOn`; no production head sampling.
- [ ] Collector tail-samples (errors / HTTP > 1 s / > 10 s / `sampling.keep` / 5 %); span
      metrics before sampling; traces 7 d, logs 7 d (longer per product when justified).
- [ ] No span outlives 60 s: long work is linked short roots per phase, end span carries
      `duration_ms`; no loop/supervisor/connection spans; per-item work DEBUG.
- [ ] No `span.enter()` across `.await`; long-lived spawns start a new root.
- [ ] Span names explicit `<component>.<unit>[.<phase>]`, no ids; `http.request` with
      `otel.name = "{METHOD} {route}"`; handlers `api.<resource>.<op>`; client spans named
      for the peer.
- [ ] Expected outcomes INFO/DEBUG; 5xx → ERROR + `error.type`, 4xx not; one log per failure.
- [ ] Correlation ids on spans **and** events; one name per concept; `outcome` on units;
      `skip_all` default.
- [ ] No bodies, query strings, tokens, signed URLs (incl. error `Display` →
      `without_url()`), argv or free text on spans/logs.
- [ ] Clients send `traceparent` + `x-<product>-*` to the own origin only; server echoes
      `x-<product>-trace-id` (CORS-exposed) and `traceId` in error bodies; clients show `Ref:`.
- [ ] Collector: `memory_limiter` first, pinned images, self-metrics scraped + alerted
      (refused/failed export, Tempo blocklist errors); span-metrics labels cleaned.
- [ ] Grafana trace ↔ log ↔ metric links provisioned.
