# Alerting

## Rule

**Every product has alert rules, kept as code, that turn a sustained failure into a
notification.** Logging an ERROR tells nobody. Rules are Grafana-provisioned (not
Alertmanager), delivered by the homelab's contact points, and each one names the incident
or failure mode it exists for.

## Why

A courier's archive job failed with ENOSPC every day for a week and deleted nothing while
the volume climbed to 94 %. The failure was in three places (the logs, a gauge, a
dashboard panel) and none of them told anyone. The gap was never instrumentation; nothing
turned a repeated ERROR into a notification.

## Where rules live

- **Code-as-truth, provisioned read-only.** A product's rules are a Grafana alerting
  provisioning file (`apiVersion: 1`, one group per product). Where the product's stack
  runs on the monitoring host, the rules live in the product repo
  (`deploy/monitoring/grafana/alerting/<product>.alerts.yml`) and the monitoring stack
  mounts them from the stack's checkout, as dashboards are
  ([grafana-dashboards.md](grafana-dashboards.md#delivery)). Otherwise they live in the
  homelab repo's `monitoring/grafana/provisioning/alerting/<product>.yml`.
- **A mirrored copy has a drift check.** When a file must be copied between repos, the
  owning repo ships `scripts/monitoring-mirror.sh check|sync`, and `check` exits non-zero on
  any difference. Unchecked mirrors drift for weeks.
- **Grafana reads alert provisioning only at boot.** A rule change needs a monitoring
  redeploy (dashboards reload on their own; rules don't).

## Rule shape

- **Labels:** `severity` (`warning` | `critical`) and `stack: <product>`, which the
  product dashboard's alert-list panel filters on. Routing is the homelab's job.
- **Annotations:** `value` (the number, formatted), `summary` (the first thing to check, one
  line), `description` (what it measures, in terms of the metric), `dashboard` (a link).
- **The file's header comment lists each rule and the incident behind it.** A rule nobody
  can explain gets deleted the first time it's noisy.
- **`noDataState` is chosen per rule.** `Alerting` for liveness (`up`), where silence is the
  failure; `OK` for a rate guard whose series legitimately vanishes.

## What makes a good rule

- **Alert on repetition, not on one error.** One failed upload is normal and retries;
  errors sustained for an hour mean the pipeline is stuck. Use a `for:` and a window.
- **Match on severity or an exact message, never a substring of a message that retries
  also log.** `|= "upload failed"` counted every retried attempt as a failure; the give-up
  is one ERROR with its own message (or `outcome = "failed"`), and that is what to count.
- **A missing series can't fire.** Pre-seed the label values a rule reads
  ([observability.md](observability.md#metrics)), and give the inputs a liveness rule of
  their own.
- **Use the leading indicator when there is one.** Free space is lagging; the floor of a
  staging directory's sawtooth ratchets up days earlier. Measure a sawtooth's floor with
  `min_over_time`, never `deriv`: a slope over a sawtooth depends on where the window
  starts.
- **Fire with runway.** A rule watching a limit the product enforces itself fires before
  the limit (at half of it), so it's a warning someone can act on, not a notice that it
  already stopped.
- **Verify the chain once.** Break the thing on purpose (point an endpoint at an
  unreachable host for one cycle) and see the notification arrive.

## Checklist

- [ ] The product has a provisioned alert rule file with a header listing each rule's reason.
- [ ] At least a liveness rule (`up{job="<product>"}`, `noDataState: Alerting`).
- [ ] Rules on sustained failure of each background pipeline, matched on severity/outcome.
- [ ] Labels `severity` + `stack`; annotations `value`, `summary`, `description`, `dashboard`.
- [ ] Any mirrored copy has a `check` that fails on drift.
