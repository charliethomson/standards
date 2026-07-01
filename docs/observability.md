# Observability

## Rule

Services initialise telemetry with **`liblog`**, export over **OTLP** to the homelab collector,
and identify themselves by their `dev.thmsn.<product>.<service>` name. **Log structured fields
— ids, counts, timings — never message bodies or query strings.** Metrics are exposed in
Prometheus format at `/api/metrics`.

## Bootstrap

Register the product name, then init `liblog`, then log startup; flush on exit:

```rust
product_name!("dev.thmsn.someproduct.api");          // becomes OTel service.name

let otlp = std::env::var("OTLP_ENDPOINT").ok();
let production = std::env::var("PRODUCTION").is_ok();
let sample_rate = std::env::var("SAMPLE_RATE").ok().and_then(|s| s.parse().ok())
    .unwrap_or(if production { 0.1 } else { 1.0 });

let guard = liblog::builder()
    .endpoint(otlp.as_deref())
    .sample_rate(sample_rate)
    .deployment_environment(if production { "production" } else { "development" })
    .init();

tracing::info!(version = env!("APP_VERSION"), "starting someproduct-api");
// ... run ...
liblog::force_cleanup(guard);                    // flush OTLP before exit
```

`liblog` fans out to: pretty **stdout** (suppressible), a size-capped **JSON file** (under
`libpath::logs_root()` — set `LIBPATH_BASE_DIR` to redirect it off the OS user dir in a
container, see [configuration.md](configuration.md)), and **OTLP** (traces + logs, batched).
Resource attributes come from
[`libproduct`](lib-ecosystem.md)/[`libbuildinfo`](build-info.md): `service.name`,
`service.version`, `deployment.environment.name`, `service.build_info` (commit + dirty flag).
Template: [`templates/rust/observability.rs`](../templates/rust/observability.rs).

## The pipeline

```
service ──OTLP gRPC :4317 / HTTP :4318──► otelcol ──► Loki (logs)
                                                  └──► Tempo (traces)
Prometheus ──scrape /api/metrics──────────────────► (metrics)   → Grafana
```

- **`OTLP_ENDPOINT`** unset → stdout + JSON only; set → export to the homelab otelcol.
- **Sampling:** `SAMPLE_RATE` overrides; default 10% in production, 100% in dev
  (`ParentBased(TraceIdRatioBased)`).
- Telemetry env vars are bare/shared ([configuration.md](configuration.md)).

This is the *emit* side. Reading it back — dashboards over these metrics/logs/traces —
is [grafana-dashboards.md](grafana-dashboards.md).

## Structured logging

- Emit **fields, not interpolated strings**: `tracing::info!(method=%m, path=%p, status, duration_ms, "http completed")`.
- Wrap requests in a span: `info_span!("http.request", method=%m, path=%p)` so downstream work
  nests under it for tracing.
- Instrument functions with `#[tracing::instrument(skip_all, fields(...))]`.
- Errors derive `valuable::Valuable` so they log as structured fields
  ([error-handling.md](error-handling.md)). This needs `rustflags = ["--cfg","tracing_unstable"]`
  in `.cargo/config.toml` and `tracing`'s `valuable` feature.

## Privacy (non-negotiable)

**Never log:** request/response bodies (carry user content), or query strings (carry `?token=`
and other secrets). **Do log:** method, route path, status, duration, outcome, entity ids,
counts. Path segments are route names/ids, not user prose.

## Metrics

Expose a `/api/metrics` poem handler (`text/plain; version=0.0.4`) backed by `prometheus-client`.
Define counters/gauges/histograms with label families, `<product>_*` named (e.g.
`someproduct_jobs_started`). Recompute snapshot gauges from in-memory state immediately before
rendering. Add a scrape target in the homelab `monitoring/prometheus` config.

## Checklist

- [ ] `product_name!("dev.thmsn.<product>.<service>")` set before init.
- [ ] `liblog::builder()` with `OTLP_ENDPOINT`/`SAMPLE_RATE`/`PRODUCTION`; guard flushed on exit.
- [ ] Logs are structured fields; requests wrapped in spans; errors `Valuable`.
- [ ] `tracing_unstable` rustflag set in `.cargo/config.toml`.
- [ ] No bodies or query strings ever logged.
- [ ] `/api/metrics` in Prometheus format (`<product>_*`), scraped by the homelab.
