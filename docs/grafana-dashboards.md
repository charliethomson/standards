# Grafana dashboards

## Rule

Dashboards are **code-as-truth JSON, committed next to the stack they observe** —
one file per dashboard, named `<name>.dashboard.json`. You author in the Grafana UI
and **export the model back to the repo**; the checked-in JSON is the source of
truth, not the copy living in Grafana's database. Queries key off the fleet's
standard identifiers ([identifiers.md](identifiers.md)) and structured log fields
([observability.md](observability.md)), so a dashboard is portable across services
by changing names, not shape.

Shape: a `someproduct-overview.dashboard.json` living either in the homelab monitoring
stack (`monitoring/`) or in the product's own `deploy/monitoring/grafana/`.

## Where dashboards live

Co-located with the stack that owns them:

| Archetype | Path |
|---|---|
| Full-stack product | `deploy/monitoring/grafana/<name>.dashboard.json` |
| The homelab monitoring stack | `monitoring/<name>.dashboard.json` |

Every product gets at least a `<product>-overview` dashboard. Suffix is always
`.dashboard.json` so the files are greppable and (later) auto-provisionable.

## Dashboard anatomy

A committed model is a plain Grafana export with a stable top:

```jsonc
{
  "uid": "someproduct-overview", // stable kebab id — never regenerate it
  "title": "SomeProduct Overview",
  "tags": ["someproduct"],       // product slug; how dashboards are grouped/found
  "schemaVersion": 39,
  "refresh": "30s",
  "time": { "from": "now-6h", "to": "now" },
  "templating": { "list": [ /* service/method selectors */ ] },
  "panels": [ /* ... */ ]
}
```

- **`uid` is permanent.** It's how links and provisioning find the dashboard; keep
  it stable across edits (re-exporting in the UI preserves it).
- **`tags`** carry the product slug so related dashboards group together.
- **Template variables** select within a product. Use the reverse-DNS component ids
  as values — e.g. a `service` variable over
  `dev.thmsn.someproduct.server,dev.thmsn.someproduct.worker` — and reference it as
  `$service` in queries so one panel serves every component.

## Datasources

Three are auto-provisioned by the monitoring stack (`grafana/provisioning/datasources/`):
**Prometheus** (metrics), **Loki** (logs), **Tempo** (traces). Panels reference a
datasource by `uid`.

**Pin the datasource `uid`s in provisioning and reference those** — `uid: prometheus`,
`uid: loki`, `uid: tempo` — rather than the opaque per-instance ids Grafana generates
(a hash like `PBFA97CFB590B2093`). A dashboard that hardcodes a generated id works only
while it matches the live box and breaks on a rebuilt Grafana. Pin all three uids in the
provisioning file and reference those from every panel, so dashboards are portable.

## Trace ↔ log ↔ metric links

Provision the links on the datasources so every signal is one click from the other two
([tracing.md](tracing.md)). `$$` escapes Grafana's provisioning env expansion.

```yaml
# grafana/provisioning/datasources/*.yml
  - name: Loki
    uid: loki
    jsonData:
      derivedFields:                      # log → trace
        - { matcherType: label, matcherRegex: trace_id, name: TraceID,
            datasourceUid: tempo, url: "$${__value.raw}" }
        # a correlation id (structured metadata on every line) → every trace for that entity;
        # long work is many linked short traces, so this finds all of them
        - { matcherType: label, matcherRegex: order_id, name: Order traces,
            datasourceUid: tempo, url: '{ span.order_id = "$${__value.raw}" }' }
  - name: Tempo
    uid: tempo
    jsonData:
      tracesToLogsV2:                     # span → its logs, same service, ±5m
        datasourceUid: loki
        spanStartTimeShift: "-5m"
        spanEndTimeShift: "5m"
        filterByTraceID: true
        customQuery: true
        query: '{service_name="$${__span.tags["service.name"]}"} | trace_id="$${__trace.traceId}"'
      tracesToMetrics:                    # span → RED for its span name
        datasourceUid: prometheus
        queries:
          - name: p95 latency
            query: 'histogram_quantile(0.95, sum by (le) (rate(traces_span_metrics_duration_milliseconds_bucket{service_name="$${__span.tags["service.name"]}", span_name="$${__span.name}"}[5m])))'
          - name: call rate
            query: 'sum by (status_code) (rate(traces_span_metrics_calls_total{service_name="$${__span.tags["service.name"]}", span_name="$${__span.name}"}[5m]))'
      nodeGraph: { enabled: true }
```

A service map needs the collector's `servicegraph` connector; without it, don't provision
`serviceMap` (it renders empty).

## Query conventions

Panels are built on the fleet's naming, so they read the same everywhere:

- **Prometheus metrics** are product-prefixed: `someproduct_jobs_total`,
  `someproduct_http_requests_total`. Liveness is `up{job="<product>"}`, mapped to
  `UP`/`DOWN` in a background stat.
- **Loki** selects by the service identifier: `{service_name="dev.thmsn.<product>.<component>"}`
  (or `{service_name="$service"}` with the template variable). Build rates with
  `count_over_time(... [$__interval])`, break logs down `by (severity_text)`, and pull
  latency from structured fields: `quantile_over_time(0.95, ... | unwrap duration_ms ...)
  by (api_operation)`. These fields exist because services log structured
  ids/counts/timings ([observability.md](observability.md)).
- A **logs panel** filtered to `severity_text = "ERROR"` for the selected `$service`
  is the standard "recent errors" tail.
- **Span metrics** (the collector's `span_metrics` connector, computed from every span
  *before* tail sampling, so they cover 100 % of traffic):
  `traces_span_metrics_calls_total` and
  `traces_span_metrics_duration_milliseconds_{bucket,sum,count}`, labelled `service_name`,
  `span_name`, `span_kind` (`SPAN_KIND_SERVER`, …) and `status_code` (`STATUS_CODE_ERROR`,
  …). Request rate/errors/latency per route come from these (`span_name` is the
  `"{METHOD} {route}"` of `http.request` spans), not from Tempo searches, which only see the
  sampled ~5 %. Buckets are coarse (10 ms … 120 s); fine-grained latency still belongs in the
  service's own histograms.

## Panel layout

The house layout on Grafana's 24-column grid:

- A **stat row** across the top: `h: 4`, small widths (`w: 3`/`4`), the at-a-glance
  numbers (totals, liveness, error counts). Use `colorMode: background` + value
  mappings for pass/fail tiles (error counts, `up`).
- **Timeseries** below at `h: 8`, half-width (`w: 12`) or third-width (`w: 8`).
  Stack related series (`stacking.mode: normal`); step-interpolate state counts.
- Override series colours by name for severities (`ERROR`→red, `WARN`→orange).

## Delivery

Today: **author in the UI → export JSON → commit → the file is the record.** The
running dashboard is redeployed by editing/importing that JSON. Datasources are
provisioned; dashboards are not, so importing is currently a manual UI step.

**Export clean, or every save is a noisy diff.** Copy from the dashboard's **JSON Model**
tab (Settings → JSON Model), *not* "Export for sharing externally" — the latter injects
`__inputs`/`__requires` and templatizes datasources into `${DS_...}` placeholders. Then,
before committing, strip the instance/volatile fields Grafana stamps in so a diff reflects
a real change, not a re-save: null the `id`, and drop `version` and `iteration`. Keep
`uid` stable. (`schemaVersion` bumps on Grafana upgrades — accept that churn.)

**Recommended next step — provision dashboards too**, so the committed files are the
literal running state (matching how datasources already work). Add a dashboard
provider and point it at the stack's dashboards dir:

```yaml
# grafana/provisioning/dashboards/dashboards.yml
apiVersion: 1
providers:
  - name: repo
    type: file
    allowUiUpdates: false          # UI edits must round-trip through git
    options:
      path: /var/lib/grafana/dashboards
      foldersFromFilesStructure: true
```

Mount the `*.dashboard.json` files at that path in the Grafana service. With
`allowUiUpdates: false`, the repo wins: you still *edit* in the UI, but changes only
persist by exporting and committing — no drift between Grafana and git.

## Checklist

- [ ] Dashboard committed as `<name>.dashboard.json` next to its stack
      (`deploy/monitoring/grafana/` for products).
- [ ] Stable kebab `uid`; `tags` include the product slug.
- [ ] Template variable selects components by reverse-DNS id; queries use `$service`.
- [ ] Prometheus queries product-prefixed; `up{job="<product>"}` liveness tile.
- [ ] Loki queries select `service_name="dev.thmsn.<product>.<component>"` and use
      structured fields (`severity_text`, `duration_ms`, `api_operation`).
- [ ] Datasources referenced by pinned `uid`s (`prometheus`/`loki`/`tempo`), not
      per-instance generated ids.
- [ ] Datasources provision trace → logs, trace → metrics (span metrics) and log → trace
      derived fields (`trace_id` + the product's correlation id).
- [ ] RED panels use `traces_span_metrics_*`, not Tempo searches.
- [ ] Stat row (`h:4`) up top; timeseries (`h:8`) below; an ERROR logs tail.
- [ ] Exported from the JSON Model tab; `id` nulled, `version`/`iteration` dropped before commit.
