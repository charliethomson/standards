# Shared MCP servers

[Model Context Protocol](https://modelcontextprotocol.io) servers an MCP client
(Claude Code) runs locally to work against the fleet. Vendored here so any repo
carrying the `standards/` submodule can wire them up, self-contained, without
depending on another repo's checkout. One directory per server.

Consuming repos reference a server by its `standards/mcp/<name>/...` path from their
root `.mcp.json` (see [`../templates/mcp.json`](../templates/mcp.json) and
[`../docs/skills-and-mcp.md`](../docs/skills-and-mcp.md)). This repo's own root
[`.mcp.json`](../.mcp.json) wires both servers for developing `standards` itself.

## Secrets

Never committed. Both servers source `mcp/.env` at launch (wired in `.mcp.json`):

```sh
cp mcp/.env.example mcp/.env   # then fill in the tokens
```

`mcp/.env` is gitignored. Non-secret values (`GRAFANA_URL`, `KOMODO_HOST`) live in
`.mcp.json`, not in `.env`. Edits take effect when the MCP client restarts.

## grafana

Grafana's third-party [`mcp-grafana`](https://github.com/grafana/mcp-grafana)
binary — PromQL/LogQL, dashboards, datasources, alert rules against the homelab's
`monitoring/` stack. Not custom, so the binary is **gitignored** (~67M,
platform-specific) rather than committed; install it into `mcp/grafana/`:

```sh
mcp/grafana/install.sh         # go install into this dir (requires Go)
```

Create a Grafana **service account token** (Administration → Users and access →
Service accounts → add one, role *Viewer*, then *Add token*) and put it in
`mcp/.env` as `GRAFANA_SERVICE_ACCOUNT_TOKEN`. `GRAFANA_URL` is pinned in `.mcp.json`.

## komodo

Custom single-file server ([`komodo/server.py`](komodo/server.py)) wrapping the
Komodo Core API — vendored here in full. Run via `uv` (PEP 723 inline deps, no venv).

- **Read-only by default.** Exposes `list_stacks`, `get_stack`,
  `get_stacks_summary`, and a generic `komodo_read(request_type, params)`.
- Execute requests (deploy/restart/stop/sync) are **off** unless you set
  `KOMODO_ALLOW_EXECUTE=1`, which adds a `komodo_execute` tool. Kept off on purpose:
  deploys and syncs are triggered by you, not by Claude.

Create an API key/secret in the Komodo UI (Settings → API Keys) and put them in
`mcp/.env` as `KOMODO_API_KEY` / `KOMODO_API_SECRET`. `KOMODO_HOST` is pinned in
`.mcp.json`.
