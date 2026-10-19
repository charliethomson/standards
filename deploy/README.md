# deploy

**The Komodo stack that runs https://standards.thmsn.dev on the homelab dev box.**

Push to `main` → `.woodpecker/server.build.yml` pushes `ghcr.io/charliethomson/standards-server:main`
→ the stack's Watchtower pulls it within five minutes and recreates `api`. Git is the source
of truth for the stack; the one secret lives in Komodo. Pattern and rationale:
[deployment](https://standards.thmsn.dev/d/deployment).

```
 client ──HTTPS──► gateway Caddy (homelab, network_mode: host)
                      standards.thmsn.dev → 127.0.0.1:8103
                      ▼
                   proxy   (caddy:2-alpine, NO TLS, security headers)
                   └── everything ─► api  (standards-server :8080, stateless)
                   watchtower  (polls GHCR, scope "standards")
```

Differences from the template, all because the server is public, read-only and stateless
(the "Public read-only exception" in docs/deployment.md): no `web` (the server renders its
own site), no `/data` volume or backup sidecar (content is compiled into the image), no
`AUTH_ADMIN_KEY`.

## Layout

| Path | What |
|---|---|
| `compose/prod.compose.yml` | `api` + `proxy` + `watchtower`; what Komodo runs |
| `compose/Caddyfile` | the in-stack router: everything to `api`, headers, 1 MiB body cap |
| `compose/local.compose.yml` | override to run the stack locally from the working tree |
| `komodo/sync/stacks.toml` | the `standards` stack (`GHCR_TOKEN`, plus the homelab's `HOMELAB_HOST_IP` for OTLP) |
| `komodo/sync/variables.toml` | declares nothing; names the variables to create |
| `komodo/sync/servers.toml` | declares nothing; the `dev` server belongs to the homelab sync |

## Things that matter

- **Host is forwarded untouched on both hops.** `/mcp` answers 403 to a `Host` not in
  `STANDARDS_ALLOWED_HOSTS` (set in the compose file). Never add `header_up Host`.
- **No `flush_interval` for `/mcp`**: it replies `application/json`, not a stream.
- **`/api/metrics` is public.** To close it, add `handle /api/metrics { respond 404 }` above
  the catch-all in `compose/Caddyfile`.

## Run it locally

```sh
cd deploy/compose
ssh-add                                     # the build fetches the private liblog over SSH
GHCR_TOKEN=x docker compose -f prod.compose.yml -f local.compose.yml up -d --build
curl -s -H 'Host: standards.thmsn.dev' localhost:8103/manifest.json
GHCR_TOKEN=x docker compose -f prod.compose.yml -f local.compose.yml down --rmi local
```

## One-time setup (go-live)

1. Komodo: the secret Variable `GHCR_TOKEN` (read:packages PAT; reuse the shared one) and
   the ghcr.io Registry Account, then Syncs → New Sync `standards`, repo
   `charliethomson/standards`, branch `main`, resource path `deploy/komodo/sync`, Managed
   mode off.
2. Check `127.0.0.1:8103` is free on dev (`ss -ltnp | grep 8103`): a grep of the homelab
   repo can't see what is merely running.
3. The homelab repo's `gateway/Caddyfile`, then redeploy the `gateway` stack:

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

4. DNS: the name is public, unlike the `*.dev.thmsn.dev` hosts, so it needs a public route
   to the gateway, not only the tailnet AdGuard rewrite.
