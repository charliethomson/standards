# Shortcuts

How to answer the questions that get asked about **standards** over and over: what's live,
which commit the server is serving, where the logs are. One command each, so nobody
re-derives them from the workflows and compose files every session.

**If you are an agent:**

- **Read this before any environment recon.** Question about the running system (prod,
  deploys, versions, logs, metrics, health)? The answer may already be below.
- **Write to it after any environment recon.** If answering took more than a couple of
  commands and the answer is durable, append an entry in the same turn, before you report
  back.
- **Record the route, not the reading.** The endpoint, command, or mechanism; never
  "prod is on 1.4.212" or "deployed at 14:32".
- **Fix what's broken.** Ran a shortcut and it failed or the output changed? Correct the
  entry now. Bump `_Verified:_` when a shortcut works.
- **No secrets.** Reference an env var or Komodo Variable; never paste the value.

Keep it under ~15 entries. Full convention: [`docs/shortcuts.md`](docs/shortcuts.md).

---

## Is standards up to date in prod?

```sh
curl -s https://standards.thmsn.dev/manifest.json | jq -r .commit; git rev-parse origin/main
```

Same sha means prod serves main. A different sha a few minutes after a push is the image
still building; longer than that, check the latest `server.build` run, then the stack's
Watchtower logs.
`/version` has the full build info (derived version, commit time, dirty flag).

_Verified: 2026-10-19_

## Is the MCP endpoint working?

```sh
curl -s -X POST https://standards.thmsn.dev/mcp -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -H 'mcp-protocol-version: 2025-06-18' -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"version","arguments":{}}}'
```

A JSON-RPC result with `server_version` and `commit` means the tunnel, proxy and api are all
up. A 403 means the request's Host is missing from `STANDARDS_ALLOWED_HOSTS`.

_Verified: 2026-10-19_

## Where are the logs?

Grafana → Explore → Loki, `{service_name="dev.thmsn.standards.server"}` for the api (OTLP).
The sidecars log as containers: `{container=~"/standards-(proxy|cloudflared|watchtower)-1"}`.
Search terms and tool arguments are never logged.

_Verified: 2026-10-19_

## Did CI pass?

```sh
woodpecker-cli pipeline ls charliethomson/standards --limit 5
```

Then `woodpecker-cli pipeline ps charliethomson/standards <n>` for the steps and
`woodpecker-cli pipeline log show charliethomson/standards <n> <step-pid>` for one log.

_Verified: 2026-10-19_
