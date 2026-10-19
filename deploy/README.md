# deploy

**The Komodo stack that runs https://standards.thmsn.dev on the homelab dev box.**

Push to `main` → `.woodpecker/server.build.yml` pushes `ghcr.io/charliethomson/standards-server:main`
→ the stack's Watchtower pulls it within five minutes and recreates `api`. Git is the source
of truth for the stack; the secrets live in Komodo. Pattern and rationale:
[deployment](https://standards.thmsn.dev/d/deployment).

```
 client ──HTTPS──► Cloudflare edge (TLS for standards.thmsn.dev, no A record)
                      │ Cloudflare Tunnel (dialled out from dev, no open port)
                      ▼
                   cloudflared  (sidecar, token mode) ──► http://proxy:80
                      ▼
                   proxy   (caddy:2-alpine, NO TLS, security headers)
                   └── everything ─► api  (standards-server :8080, stateless)
                   watchtower  (polls GHCR, scope "standards")

 tailnet (optional) ──► gateway Caddy ──► 127.0.0.1:8103 ──► proxy
```

The public route is a Cloudflare Tunnel run by the stack's own `cloudflared`, the same
shape as the homelab's `ci.thmsn.dev` (homelab `woodpecker/`). There is no public A record
and nothing listens on a public port. The homelab gateway is not on the public path; its
block is kept below as an optional tailnet route.

Differences from the template, all because the server is public, read-only and stateless
(the "Public read-only exception" in docs/deployment.md): no `web` (the server renders its
own site), no `/data` volume or backup sidecar (content is compiled into the image), no
`AUTH_ADMIN_KEY`, and a `cloudflared` sidecar for the public route.

## Layout

| Path | What |
|---|---|
| `compose/prod.compose.yml` | `api` + `proxy` + `cloudflared` + `watchtower`; what Komodo runs |
| `compose/Caddyfile` | the in-stack router: everything to `api`, headers, 1 MiB body cap |
| `compose/local.compose.yml` | override to run the stack locally from the working tree, without `cloudflared` or `watchtower` |
| `komodo/sync/stacks.toml` | the `standards` stack (`GHCR_TOKEN`, `STANDARDS_CF_TUNNEL_TOKEN`, plus the homelab's `HOMELAB_HOST_IP` for OTLP) |
| `komodo/sync/variables.toml` | declares nothing; names the variables to create |
| `komodo/sync/servers.toml` | declares nothing; the `dev` server belongs to the homelab sync |

## Things that matter

- **Host is forwarded untouched on every hop.** `/mcp` answers 403 to a `Host` not in
  `STANDARDS_ALLOWED_HOSTS` (set in the compose file). cloudflared keeps the client's
  `Host` unless the tunnel route sets an HTTP Host Header, so leave that field empty in
  Zero Trust, and never add `header_up Host` in Caddy.
- **The tunnel targets `proxy:80`, not `api:8080`.** Going through Caddy is what applies
  the security headers and the body cap.
- **The tunnel token is its own Komodo Variable**, `STANDARDS_CF_TUNNEL_TOKEN`, written
  into the stack `.env` as `CF_TUNNEL_TOKEN`. The homelab's `CF_TUNNEL_TOKEN` Variable is
  the `ci.thmsn.dev` tunnel: reusing it would make this stack a second connector of that
  tunnel, and Cloudflare would send it a share of the ci traffic.
- **No `flush_interval` for `/mcp`**: it replies `application/json`, not a stream.
- **`/api/metrics` is public.** To close it, add `handle /api/metrics { respond 404 }` above
  the catch-all in `compose/Caddyfile`.

## Run it locally

`cloudflared` is left out (the override puts it in the `prod-only` profile): there is no
token locally. The loopback port stands in for the public route.

```sh
cd deploy/compose
ssh-add                                     # the build fetches the private liblog over SSH
GHCR_TOKEN=x docker compose -f prod.compose.yml -f local.compose.yml up -d --build
curl -s -H 'Host: standards.thmsn.dev' localhost:8103/manifest.json
GHCR_TOKEN=x docker compose -f prod.compose.yml -f local.compose.yml down --rmi local
```

## One-time setup (go-live)

Cloudflare first, so the sidecar has a token when the stack first starts (without one it
restarts in a loop).

1. **Tunnel.** Cloudflare Zero Trust → Networks → Tunnels → Create a tunnel → Cloudflared,
   name `standards`. Copy the token from the install command (the string after
   `--token`). Skip installing a connector: the stack runs it.
2. **Public hostname.** On that tunnel, add a published application route: subdomain
   `standards`, domain `thmsn.dev`, no path, service type `HTTP`, URL `proxy:80`. Leave
   HTTP Host Header empty. Cloudflare creates the proxied `standards` CNAME to the tunnel
   itself; if an A, AAAA or CNAME record for `standards.thmsn.dev` already exists, delete
   it first. Do not add an A record.
3. **WAF and Access: none.** The server is public and read-only, so it gets no Access
   application and no WAF rule. Check that no existing Access application matches the
   name. Optional: a rate limiting rule on `standards.thmsn.dev` (for example on `/mcp`)
   if it ever draws abuse.
4. **Komodo.** Secret Variables `GHCR_TOKEN` (read:packages PAT; reuse the shared one) and
   `STANDARDS_CF_TUNNEL_TOKEN` (the token from step 1), and the ghcr.io Registry Account.
   Then Syncs → New Sync `standards`, repo `charliethomson/standards`, branch `main`,
   resource path `deploy/komodo/sync`, Managed mode off.
5. **Check.** Zero Trust shows the `standards` tunnel Healthy, and from anywhere:

   ```sh
   curl -s https://standards.thmsn.dev/health
   curl -s -X POST https://standards.thmsn.dev/mcp \
     -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
     -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"0"}}}'
   ```

   A 1033 means the tunnel is down (`docker logs standards-cloudflared-1` on dev). A 502
   means the route points somewhere other than `proxy:80`. A 403 from `/mcp` means the
   route overrides the Host header.

### Optional: tailnet route through the gateway

Not needed for go-live. It lets tailnet clients reach the stack without going through
Cloudflare, and only takes effect if the tailnet's AdGuard rewrites `standards.thmsn.dev`
to the gateway. The name is outside the `*.dev.thmsn.dev` wildcard, so by default tailnet
clients resolve it to Cloudflare like everyone else. This route is why `proxy` keeps its
loopback port `127.0.0.1:8103` (local runs use it too). To use it, check the port is free
on dev (`ss -ltnp | grep 8103`), add this block to the homelab repo's `gateway/Caddyfile`,
redeploy the `gateway` stack, and add the AdGuard rewrite:

```caddy
standards.thmsn.dev {
    tls {
        dns cloudflare {env.CF_API_TOKEN}
        resolvers 1.1.1.1 8.8.8.8
    }
    header -Server
    reverse_proxy localhost:8103 {
        header_up X-Real-IP {remote_host}
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
}
```
