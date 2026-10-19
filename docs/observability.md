# Observability

## Rule

Services initialise telemetry with **`liblog`**, export over **OTLP** to the homelab collector,
and identify themselves by their `dev.thmsn.<product>.<service>` name. **Log structured fields
— ids, counts, timings — never message bodies or query strings.** Metrics are exposed in
Prometheus format at `/api/metrics`.

**A component whose stdout is a wire logs to stderr** — `liblog` is still mandatory, but it is
initialised with `.console_stream(liblog::ConsoleStream::Stderr)`. See
[When stdout is the wire](#when-stdout-is-the-wire). This is the one carve-out; there is no
component exempt from initialising `liblog` at all.

## Bootstrap

Declare the product name, **register it globally**, then init `liblog`, then log startup;
flush on exit. liblog reads `service.name` from the global product name, so skipping
`set_global()` exports an empty `service.name`:

```rust
product_name!("dev.thmsn.someproduct.api");          // declares a const — nothing is live yet

// Publish it. The macro only *declares*; until set_global() runs,
// ProductName::global() is None, the OTel service.name falls back, and every
// libpath-derived root (logs, config, cache) lands under dev.thmsn.unspecified.
// Bind to a local first: product_name! expands to a `const` holding a LazyLock,
// so borrowing it inline acts on a fresh copy and trips
// clippy::borrow_interior_mutable_const.
let product_name = PRODUCT_NAME;
product_name.set_global().expect("product name already set");

let otlp = std::env::var("OTLP_ENDPOINT").ok();
let production = std::env::var("PRODUCTION")
    .is_ok_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
let sampling = match std::env::var("SAMPLE_RATE").ok().and_then(|s| s.parse().ok()) {
    Some(rate) => liblog::Sampling::Ratio(rate),  // explicit override only
    None => liblog::Sampling::AlwaysOn,           // the collector tail-samples
};

let guard = liblog::builder()
    .endpoint(otlp.as_deref())
    .sampling(sampling)
    .deployment_environment(if production { "production" } else { "development" })
    .init();

tracing::info!(version = env!("APP_VERSION"), ?sampling, "starting someproduct-api");
// ... run ...
liblog::force_cleanup(guard);                    // flush OTLP before exit
```

`liblog` fans out to: a pretty **console** layer (stdout by default, suppressible with
`.quiet(true)`, redirectable with `.console_stream(..)`), a size-capped **JSON file** (under
`libpath::logs_root()` — set `LIBPATH_BASE_DIR` to redirect it off the OS user dir in a
container, see [configuration.md](configuration.md)), and **OTLP** (traces + logs, batched).
Resource attributes come from
[`libproduct`](lib-ecosystem.md)/[`libbuildinfo`](build-info.md): `service.name`,
`service.version`, `deployment.environment.name`, `service.build_info` (commit + dirty flag),
plus `host.name` and `service.instance.id` (from `HOSTNAME`).
Template: [`templates/rust/observability.rs`](../templates/rust/observability.rs).

## When stdout is the wire

Some components speak a protocol over stdin/stdout — an **MCP server on the stdio transport**
is the standard case; so is any CLI whose stdout is designed to be piped into another program.
For these, **stdout is not output, it is the transport**, and the console layer's first record
is interleaved into the frame stream:

```rust
let guard = liblog::builder()
    .endpoint(otlp.as_deref())
    .console_stream(liblog::ConsoleStream::Stderr)   // stdout carries JSON-RPC
    .init();
```

Everything else is unchanged: the JSON file and OTLP layers are untouched, and the component
is as observable as any other.

**Why this is a rule and not a preference.** The failure is silent and total. Nothing errors —
the peer's parser meets a frame it can neither read nor attribute, the session stops making
sense, and the symptom surfaces in whatever client is on the other end, a long way from the
cause. `.quiet(true)` is not the fix: it protects the wire by giving up human-readable logs
entirely, which is why `liblog` grew a redirect instead.

**Nothing else may write to stdout either** — no `println!`, no `dbg!`, no dependency that
logs helpfully on its own. Reading the code cannot prove the absence of a stray write, so
**prove it by mutation**: inject a single `println!` into the handshake path and confirm the
protocol tests fail. A test suite that passes both with and without that `println!` is not
testing the wire.

## The pipeline

```
service ──OTLP gRPC :4317 / HTTP :4318──► otelcol ──► Loki (logs)
                                                  └──► Tempo (traces)
Prometheus ──scrape /api/metrics──────────────────► (metrics)   → Grafana
```

- **`OTLP_ENDPOINT`** unset → stdout + JSON only; set → export to the homelab otelcol.
- **Sampling: services are always-on; the collector tail-samples** (errors, slow requests,
  `sampling.keep`, a 5 % baseline) with span metrics computed before sampling. Rules, span
  shape, names and the client contract: [tracing.md](tracing.md). `SAMPLE_RATE` (a
  parent-based ratio) and the spec's `OTEL_TRACES_SAMPLER` / `OTEL_TRACES_SAMPLER_ARG`
  (`always_on`, `parentbased_always_on`, `traceidratio`, `parentbased_traceidratio`,
  `always_off`) are overrides for local work, never the production default, and stay out of
  prod compose files (setting `always_on` there only restates the default). Env beats the
  builder. liblog has no true off switch: `traceidratio` is parent-based like
  `parentbased_traceidratio`, and `always_off` is a parent-based ratio of 0, so a request
  arriving with a sampled parent is still sampled.
- Telemetry env vars are bare/shared ([configuration.md](configuration.md)).
- **Logs are stored once.** A service that exports logs over OTLP carries the
  `dev.thmsn.telemetry=otlp` label on its prod compose service; the host's docker log shipper
  drops labelled containers, so those logs reach Loki only via OTLP. Anything printed before
  the exporter starts (startup panics, config errors) stays in `docker logs` only.

Client usage events ride the same OTLP log path from the product server, and the collector
routes them to ClickHouse instead of Loki: [client-analytics.md](client-analytics.md).

This is the *emit* side. Reading it back — dashboards over these metrics/logs/traces —
is [grafana-dashboards.md](grafana-dashboards.md).

## Filters

Each liblog layer has its own `EnvFilter`, overridable from env without a rebuild (standard
`RUST_LOG` directive syntax; an invalid value warns on stderr and falls back):

| Env var | Layer | Default |
|---|---|---|
| `LIBLOG_OTEL_FILTER` | OTLP spans | `info` + `NOISY_TARGETS` |
| `LIBLOG_LOG_FILTER` | OTLP log bridge (→ Loki) | `info` + `NOISY_TARGETS` |
| `LIBLOG_JSON_FILTER` | JSON file | `info` |
| `LIBLOG_STDOUT_FILTER`, then `RUST_LOG` | console | `info` |

`NOISY_TARGETS` (liblog `filter.rs`, the source of truth) is
`h2=off,hyper=warn,hyper_util=warn,tonic=warn,tower=warn,reqwest=warn,reqwest_tracing=info,rustls=warn,sqlx=warn,opentelemetry*=off,chromiumoxide=error`.
`h2` is off because it opens an unbounded root span per HTTP/2 connection;
`reqwest_tracing=info` keeps HTTP client spans that `reqwest=warn` would otherwise silence.

**An env override replaces the default wholesale.** Only the `opentelemetry*=off`
self-silence is re-appended, so `LIBLOG_OTEL_FILTER=info,somecrate=debug` turns `h2`'s
connection spans back on. To look at one crate's DEBUG in prod, repeat the list:
`LIBLOG_OTEL_FILTER="info,$NOISY_TARGETS,somecrate=debug"` (paste the value above).
`Builder::target_level(target, level)` is a global cap that holds even over env.
`OTEL_BSP_*` / `OTEL_BLRP_*` tune the batch exporters; always-on services set
`OTEL_BSP_MAX_QUEUE_SIZE=8192` ([tracing.md](tracing.md#pipeline-hygiene)).

Setting a filter in code is for a documented need, with a comment naming it: keeping
`client_event` records out of the console and JSON file
([client-analytics.md](client-analytics.md)), or capping a dependency that floods every
layer. Tuning for taste belongs in env.

## Propagation

liblog has a helper for each side of a hop:

| Situation | Call |
|---|---|
| Inbound request from an app, browser or CLI | your own `http.request` span, then `adopt_client_trace(&span, &extract_context(&HeaderExtractor(req.headers())))`: same trace id, no parent, a link to the client's span |
| Inbound request from another fleet service | `server_span!(&HeaderExtractor(req.headers()), method, route, ..)`: a child of the caller's CLIENT span |
| Outbound call to anything | `client_span!("peer.op", peer.service = .., server.address = ..)` |
| Outbound call to a fleet service | the same, plus `inject_span(&span, &mut HeaderInjector(&mut headers))` |
| Linking a new root to the work that caused it | `span.add_link(current_span_context())` |

`extract_context` / `inject_context` remain for custom carriers (an RPC envelope's
`traceparent` field). The server/client contract is in
[tracing.md](tracing.md#client--server-contract).

## Structured logging

- Emit **fields, not interpolated strings**: `tracing::info!(method=%m, path=%p, status, duration_ms, "http completed")`.
- Wrap requests in an `http.request` span (`otel.name = "{METHOD} {route}"`) so downstream work
  nests under it; span naming, levels and attributes are in [tracing.md](tracing.md).
- Instrument functions with `#[tracing::instrument(skip_all, fields(...))]`.
- Errors derive `valuable::Valuable` so they log as structured fields
  ([error-handling.md](error-handling.md)). This needs `rustflags = ["--cfg","tracing_unstable"]`
  in `.cargo/config.toml` and `tracing`'s `valuable` feature. A `Valuable` value reaches Loki
  as one blob, so anything a query filters or groups on is **also** logged as its own flat
  field: `error.type` next to the error, `result_kind` next to the result.

## Privacy (non-negotiable)

**Never log:** request/response bodies (carry user content), or query strings (carry `?token=`
and other secrets). **Do log:** method, route path, status, duration, outcome, entity ids,
counts. Path segments are route names/ids, not user prose.

## Metrics

Expose a `/api/metrics` poem handler backed by **`prometheus-client`**, served as

```
application/openmetrics-text; version=1.0.0; charset=utf-8
```

Define counters/gauges/histograms with label families, `<product>_*` named (e.g.
`someproduct_jobs_started`). Recompute snapshot gauges from in-memory state immediately before
rendering. Add a scrape target in the homelab `monitoring/prometheus` config.

- **HTTP metrics come from the outermost layer:** `<product>_http_requests_total{method,route,status}`
  and `<product>_http_request_duration_seconds{method,route}`. Outermost, so they count error
  responses, panics and the DEBUG hot paths that span metrics never see. Label values come
  from bounded sets: the route **template**, `unmatched` for a router miss, `other` for an
  unknown method. For a streamed body the duration is time to headers; say so in the help text.
- **Every service exports `<product>_build_info{version,git_hash,...} 1` and
  `<product>_process_start_time_seconds`**, so a dashboard can mark deploys and show what's live.
- **Pre-seed known label values** (each status class, each outcome) so a series exists at
  zero; an alert on a series that has never been written can't fire.
- **Test that `/api/metrics` renders a sample for every family.** A registration macro that
  evaluated its argument twice once left every counter rendering an empty family for months.

> **It is OpenMetrics, not `text/plain; version=0.0.4`.** `prometheus-client`'s only text
> encoder (`prometheus_client::encoding::text`) emits the OpenMetrics text format — `_total`
> counter suffixes and a trailing `# EOF` — and the crate ships no Prometheus-0.0.4 encoder to
> switch to. Prometheus has negotiated and parsed OpenMetrics since 2.x, so this is the format
> to *declare*; labelling the same bytes `text/plain` would work only by accident, with `# EOF`
> swallowed as a comment. The crate exports no content-type constant, so declare the one above
> yourself, next to the handler.

## Checklist

- [ ] `product_name!("dev.thmsn.<product>.<service>")` **declared and published via
      `set_global()`** before init. The macro alone only declares it (else `service.name` is empty).
- [ ] `liblog::builder()` with `OTLP_ENDPOINT`, `PRODUCTION` parsed as a bool (not
      `is_ok()`), `Sampling::AlwaysOn` unless `SAMPLE_RATE` overrides; guard flushed on exit.
- [ ] Components whose stdout is a protocol stream init with
      `.console_stream(ConsoleStream::Stderr)`, and nothing anywhere writes to stdout,
      proven by injecting a `println!` and watching the protocol tests fail.
- [ ] Per-layer filters left at defaults in code unless a commented need says otherwise;
      tuned via `LIBLOG_*_FILTER` env, repeating `NOISY_TARGETS` in any override.
- [ ] Tracing follows [tracing.md](tracing.md) (its own checklist).
- [ ] Logs are structured fields; requests wrapped in spans; errors `Valuable`.
- [ ] `tracing_unstable` rustflag set in `.cargo/config.toml`.
- [ ] Every OTLP-exporting compose service has the `dev.thmsn.telemetry=otlp` label.
- [ ] No bodies or query strings ever logged.
- [ ] `/api/metrics` served as `application/openmetrics-text; version=1.0.0` (`<product>_*`),
      scraped by the homelab; HTTP metrics from the outermost layer with bounded labels;
      `_build_info` and `_process_start_time_seconds`; label values pre-seeded; a render test.
- [ ] Alerts follow [alerting.md](alerting.md).
