---
name: thmsn-logs
description: Query a deployed product's logs, metrics and traces in the homelab Grafana (Loki / Prometheus / Tempo) — sweep for recurring errors, follow one request by trace id, check what build is live, read latency and error rates. Use whenever investigating prod behaviour — "check the logs", "what's erroring", "any errors this week", "query Loki/Grafana", "find this Ref:", "is it slow", "what's deployed". Reads the repo's own specialisation from .standards.d/thmsn-logs/.
---

# Query a product's observability (Grafana / Loki / Prometheus / Tempo)

Every fleet service ships logs and traces over OTLP and exposes Prometheus metrics
([`docs/observability.md`](../../docs/observability.md), [`docs/tracing.md`](../../docs/tracing.md)).
All of it lands in the homelab Grafana at `https://grafana.dev.thmsn.dev`. Query it through
the bundled helper, which talks to Grafana's datasource proxy with a **read-only**
service-account token, so it needs no ssh and can't change anything.

## Start here

Run from the repo root (the script finds the repo with `git`, so any subdirectory works):

```bash
S=standards/skills/thmsn-logs/scripts/obsq.sh
$S env                   # which services, which selector, is the token found
$S recipes               # named queries: this repo's first, then the fleet's
cat .standards.d/thmsn-logs/NOTES.md   # this repo's fields, gotchas and known noise
```

**Read the repo's `NOTES.md` before drawing conclusions.** It names what the fleet skill
can't know: which fields its lines carry, which warnings are expected noise, and where
its logging has gaps (a request path that never logs errors makes "no errors" meaningless).
No `.standards.d/thmsn-logs/`? `$S init` scaffolds one; fill in `SERVICES`.

## The helper

| Subcommand | What it does |
|---|---|
| `logs <logql> [since] [until] [limit]` | Matching lines, oldest first: `time [LEVEL] [scope] body`. Takes the newest `limit` (100). |
| `meta <logql> [since] [until] [limit]` | Full structured metadata of the matches (3). How you find a line's fields. |
| `top <logql> [since] [until] [limit]` | Matches grouped by level + scope + body, ids and numbers folded, commonest first, with one sample `error` and when it last happened. Default since is 7 days. The sweep tool. |
| `series <logql> [at]` | Instant LogQL aggregation (`count_over_time`, `quantile_over_time`, …). |
| `prom <promql> [at]` | Instant PromQL. |
| `trace <trace-id>` | One trace from Tempo as time-sorted span lines, failures marked `ERROR`. |
| `ref <hex prefix> [since] [until]` | A client's `Ref:` (the first hex of `x-<product>-trace-id`) → full trace ids. |
| `run <recipe> [--range 7d] [args…]` | A named query from `queries/`. |
| `<command> [args…]` | A repo command from `.standards.d/thmsn-logs/commands/`. |

In any query `{{sel}}` is the repo's stream selector and `{{svc}}` its first service, so
the same text works in every repo: `$S logs '{{sel}} | severity_text=`ERROR`' '6 hours ago'`.
Times are GNU-date expressions (`'30 minutes ago'`, `'2026-06-10T05:00:00Z'`) or epoch
seconds. Needs `curl`, `jq` and GNU date (`gdate` from coreutils on macOS).

## Fleet recipes

| Recipe | Answers |
|---|---|
| `problems` | Every WARN and ERROR over the week, grouped. **Start an error sweep here.** |
| `errors` | The ERROR lines themselves. |
| `levels` | Line counts by service and level (`--range 7d` for the week). |
| `http-failures` | 4xx/5xx by `api_operation` and `status`. |
| `http-5xx` | The 5xx lines; `meta` them for `trace_id`. |
| `latency-p95` | p95 `duration_ms` by `api_operation`. |
| `build` | The newest line's metadata: `service_build_info`, environment. |
| `search <text>` | Lines containing the text. |
| `up <job>` | Prometheus scrape health for a job. |

## Sweeping for recurring errors

1. `$S run levels --range 7d`: is there anything above INFO at all.
2. `$S run problems`: what, how often, how recently. Check each group against the
   repo's known noise.
3. For a group worth chasing, how it spreads over time:
   `$S series 'sum(count_over_time({{sel}} |= `<message>` [1d]))' '<n> days ago'` per day,
   or a `[1h]` window for a burst.
4. `$S meta '{{sel}} |= `<message>`' '7 days ago' now 3` for its fields, then the code that
   logs it (`scope_name` is the Rust module path).
5. Logs show what was logged. A failure that is retried or only counted is often logged
   once per give-up, not per attempt; the repo's NOTES say where the database holds the
   rest.

## Gotchas

1. **Per-request and per-entity fields are structured metadata, not indexed labels.**
   Bodies are terse ("http completed"); filter with a pipeline stage, `| job_id=`x``, not
   a line filter. The indexed labels are few (`service_name`, `level`, `container`, `host`, …); `meta` shows both kinds merged, so filter by pipeline stage unless it is one of those.
2. **DEBUG and TRACE never reach Loki or Tempo.** The OTLP bridges filter at INFO
   (`LIBLOG_LOG_FILTER` / `LIBLOG_OTEL_FILTER` change it without a rebuild), so a missing
   debug line proves nothing.
3. **Expected outcomes are INFO** (tracing.md): 4xx and not-found are not WARN. A clean
   `problems` is not a clean service; check `http-failures` too.
4. **Retention is 7 days.** Anything older is gone from Loki; a product's database or Prometheus
   (kept longer) may still have it.
5. **Pre-exporter output is only in `docker logs`.** A service that sets the
   `dev.thmsn.telemetry=otlp` compose label is dropped by the host shipper, so a startup
   panic printed before the exporter came up never reaches Loki.
6. **Loki renames and drops fields.** Structured metadata turns dots into underscores
   (`error.type` → `error_type`, `enduser.id` → `enduser_id`) and drops empty values, so
   a field that is "always there" in the code may be absent on a line.
7. **`=~` is anchored in Loki.** `| trace_id=~`abc`` matches nothing; write `^abc.*`.

## Specialising it for a repo: `.standards.d/thmsn-logs/`

The skill is the same in every repo; what differs lives in the consuming repo, next to
the code it describes. `obsq.sh` reads it on every run.

```
.standards.d/thmsn-logs/
├── config        # KEY=VALUE, sourced after .standards.conf
├── NOTES.md      # this repo's fields, gaps, known noise, recipe index; agents read it
├── queries/      # <name>.logql | <name>.promql; a repo recipe shadows a fleet one of the same name
└── commands/     # executables that become `obsq.sh <name>` subcommands
```

- **`config`**: `SERVICES` (space-separated `service_name`s; `{{sel}}` matches all of them),
  or a whole `SELECTOR`; `GRAFANA_URL`, `GRAFANA_ENV_FILE`, `GRAFANA_TOKEN_VAR` and the
  datasource UIDs only when this product's telemetry goes somewhere else. Unset, the
  selector is every `dev.thmsn.<PRODUCT>.*` service from `.standards.conf`. No secrets.
- **Recipes** are the query with `#` header lines: `desc` (shown by `recipes`), `kind`
  (`logs`, `meta`, `top`, `series`, `prom`), `range` (fills `{{range}}`; also the look-back
  when there is no `since`), `since`, `limit`. Positional args fill `{{1}}`…`{{9}}`.

  ```
  # desc: summaries parked after repeated failures
  # kind: top
  # range: 7d
  {{sel}} |= `will not be enqueued again`
  ```
- **Commands** are for what a query can't say: a read-only database query over ssh, a
  join of two queries, a product API call. They run with `OBSQ_TOKEN`, `OBSQ_LOKI`,
  `OBSQ_PROM`, `OBSQ_TEMPO`, `OBSQ_SELECTOR`, `OBSQ_SERVICE`, `OBSQ_REPO` and
  `OBSQ_SCRIPT` (call it back for `logs`/`series`) in the environment. A `# desc:` line is
  shown by `recipes`. Read-only, always.

**When you learn something about this repo's logs, write it into the overlay**: a field,
a gap, a message that turned out to be noise, a query you had to work out. A query you
ran twice is a recipe. Something every product would want belongs in this skill instead
(`thmsn-standards-contribute`).

Raw access, if the helper is in the way: `https://grafana.dev.thmsn.dev/api/datasources/proxy/uid/<uid>/…`
with `Authorization: Bearer <token>`; Loki `loki` (`/loki/api/v1`), Prometheus `prometheus`
(`/api/v1`), Tempo `tempo` (`/api`).

**The token** is a read-only Grafana service-account token, `GRAFANA_SERVICE_ACCOUNT_TOKEN`.
The helper takes the env var, else the first of: the overlay's `GRAFANA_ENV_FILE`, the
standards checkout's gitignored `mcp/.env` (the same file the Grafana MCP server reads,
[`mcp/README.md`](../../mcp/README.md)), `<repo>/standards/mcp/.env`,
`~/git/standards/mcp/.env`. `$S env` says which it used.
