# Observability

## Rule

Services initialise telemetry with **`liblog`**, export over **OTLP** to the homelab collector,
and identify themselves by their `dev.thmsn.<product>.<service>` name. **Log structured fields
— ids, counts, timings — never message bodies or query strings.** Metrics are exposed in
Prometheus format at `/api/metrics`.

## Bootstrap

Declare the product name, **register it globally**, then init `liblog`, then log startup;
flush on exit. liblog reads `service.name` from the global product name, so skipping
`set_global()` exports an empty `service.name`:

```rust
product_name!("dev.thmsn.someproduct.api");          // becomes OTel service.name

let product_name = PRODUCT_NAME;                  // a const LazyLock: bind, then borrow
product_name.set_global().expect("product name set twice");
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

`liblog` fans out to: pretty **stdout** (suppressible), a size-capped **JSON file** (under
`libpath::logs_root()` — set `LIBPATH_BASE_DIR` to redirect it off the OS user dir in a
container, see [configuration.md](configuration.md)), and **OTLP** (traces + logs, batched).
Resource attributes come from
[`libproduct`](lib-ecosystem.md)/[`libbuildinfo`](build-info.md): `service.name`,
`service.version`, `deployment.environment.name`, `service.build_info` (commit + dirty flag),
plus `host.name` and `service.instance.id` (from `HOSTNAME`).
Template: [`templates/rust/observability.rs`](../templates/rust/observability.rs).

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
  `always_off`) are overrides for local work, never the production default. Env beats the
  builder. liblog has no true off switch: `traceidratio` is parent-based like
  `parentbased_traceidratio`, and `always_off` is a parent-based ratio of 0, so a request
  arriving with a sampled parent is still sampled.
- Telemetry env vars are bare/shared ([configuration.md](configuration.md)).

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

`NOISY_TARGETS` caps `h2`, `hyper`, `tonic`, `tower`, `reqwest`, `rustls`, `sqlx` and
`opentelemetry*`. To look at one crate's DEBUG in prod, widen one layer:
`LIBLOG_OTEL_FILTER=info,somecrate=debug`. `Builder::target_level(target, level)` is a global
cap that holds even over env. `OTEL_BSP_*` / `OTEL_BLRP_*` tune the batch exporters.

## Propagation

liblog re-exports what a service needs to continue and hand on a trace:
`extract_context(&HeaderExtractor(req.headers()))` → `span.set_parent(cx)` (via
`OpenTelemetrySpanExt`) for inbound `traceparent`; `inject_context(&cx, &mut
HeaderInjector(&mut headers))` for outbound; `current_span_context()` for `span.add_link(..)` between
linked roots. The server/client contract is in [tracing.md](tracing.md#client--server-contract).

## Structured logging

- Emit **fields, not interpolated strings**: `tracing::info!(method=%m, path=%p, status, duration_ms, "http completed")`.
- Wrap requests in an `http.request` span (`otel.name = "{METHOD} {route}"`) so downstream work
  nests under it; span naming, levels and attributes are in [tracing.md](tracing.md).
- Instrument functions with `#[tracing::instrument(skip_all, fields(...))]`.
- Errors derive `valuable::Valuable` so they log as structured fields
  ([error-handling.md](error-handling.md)). This needs `rustflags = ["--cfg","tracing_unstable"]`
  in `.cargo/config.toml` and `tracing`'s `valuable` feature.

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

> **It is OpenMetrics, not `text/plain; version=0.0.4`.** `prometheus-client`'s only text
> encoder (`prometheus_client::encoding::text`) emits the OpenMetrics text format — `_total`
> counter suffixes and a trailing `# EOF` — and the crate ships no Prometheus-0.0.4 encoder to
> switch to. Prometheus has negotiated and parsed OpenMetrics since 2.x, so this is the format
> to *declare*; labelling the same bytes `text/plain` would work only by accident, with `# EOF`
> swallowed as a comment. The crate exports no content-type constant, so declare the one above
> yourself, next to the handler.

## Checklist

- [ ] `product_name!("dev.thmsn.<product>.<service>")` **and `PRODUCT_NAME.set_global()`**
      before init (else `service.name` is empty).
- [ ] `liblog::builder()` with `OTLP_ENDPOINT`, `PRODUCTION` parsed as a bool (not
      `is_ok()`), `Sampling::AlwaysOn` unless `SAMPLE_RATE` overrides; guard flushed on exit.
- [ ] Per-layer filters left at defaults in code; tuned via `LIBLOG_*_FILTER` env.
- [ ] Tracing follows [tracing.md](tracing.md) (its own checklist).
- [ ] Logs are structured fields; requests wrapped in spans; errors `Valuable`.
- [ ] `tracing_unstable` rustflag set in `.cargo/config.toml`.
- [ ] No bodies or query strings ever logged.
- [ ] `/api/metrics` served as `application/openmetrics-text; version=1.0.0` (`<product>_*`),
      scraped by the homelab.
