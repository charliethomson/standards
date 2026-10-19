# TAV-51: Tracing overhaul: connected, session-aware traces across server, WS and clients
state=backlog priority=high created=2026-09-27T05:56:01.632615888+00:00

Program parent for tavern's tracing overhaul (audit 2026-09-27). The full report, scheme and findings (file:line) are in the repo: `reviews/tracing-audit-2026-09-27.md`. Children are the phases; the shipped ones are done with their commits linked, and the open ones are the follow-ups.

**Why:** traces didn't connect (no traceparent in or out). The WebSocket had no spans at all, and generations were orphan roots. The LLM call had no span. Failures didn't mark spans as errors, so a tail sampler couldn't keep them. A 16-attribute cap was silently dropping attributes, and sqlx statement events filled the per-span event cap. There were real content leaks: upstream error bodies, URLs from chat text in reqwest errors, serde errors quoting values, and `?sig=` in Caddy error logs.

**Scheme (report §3):**
- One short span per unit of work, never per connection. WS frames are roots continuing the frame's `traceparent`, linked to the upgrade.
- Semconv attributes plus `tavern.*`: `enduser.id`, `client.*`, `session.id`, `tavern.ws.conn_id`, `tavern.outcome`.
- ERROR status for server faults only.
- Clients send `traceparent` + `X-Tavern-*` identity (the WS carries it in the auth frame) and show `Ref: <8 hex>`.
- The server echoes `X-Tavern-Trace-Id` / `traceId`.

**Shared pieces** (liblog TTR-49, collector/retention/Grafana TTR-53, auth propagation TTR-55, standard TTR-47) are owned by the ttr3 program. Tavern's requirements are in report §5 and in comments on TTR-49 and TTR-53.


