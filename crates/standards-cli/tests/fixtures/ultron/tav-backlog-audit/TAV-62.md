# TAV-62: Tracing: re-verify tavern end to end once tail sampling and retention land
state=backlog priority=high created=2026-09-27T05:56:57.394980619+00:00

Blocked by the ttr3 collector phase (TTR-53). Once otelcol tail sampling, Tempo 7d / Loki 14d retention, tracesToLogsV2 and the promtail `dev.thmsn.telemetry=otlp` drop are live, verify against prod tavern: (1) a web and an iOS error show a Ref that resolves in Tempo; (2) a failed generation's trace is kept (ws.generate -> generation -> generation.reply -> openrouter.chat, with ERROR status); (3) a slow-TTFT generation is kept by the `tavern.gen_ai.ttft_ms >= 8000` policy (tavern's requirements are in the TTR-53 comment); (4) the baseline is ~5%; (5) no duplicate tavern logs from promtail; (6) the dashboard Traces row populates. Also check that `dev.thmsn.tavern.api` is included in tail sampling rather than passed through (the TTR-53 description scopes sampling to `dev.thmsn.ttr.*`). Then follow the amended standard (TTR-47) for anything it changed.

Part of TAV-51 (report: `reviews/tracing-audit-2026-09-27.md`).


