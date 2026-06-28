# Deployment

## Rule

Services deploy to the homelab as **Komodo-managed docker-compose stacks**, behind the
homelab **gateway Caddy**, with images auto-rolled by **Watchtower**. Git is the source of
truth; secrets live in the Komodo UI, never in the repo.

Public hostname: `<product>.dev.thmsn.dev`. See [identifiers.md](identifiers.md) for the
identifier-vs-hostname distinction.

## The stack

A product's `deploy/` ships a compose stack with four services:

```
 browser ──HTTPS──► gateway Caddy (homelab, network_mode: host)
                       <product>.dev.thmsn.dev → 127.0.0.1:<loopback-port>
                       ▼
                    proxy   (caddy:2-alpine, this stack — NO TLS)
                    ├── /api/*, /ws, /health ─► api  (<product>-api, :8080, /data volume)
                    └── everything else ──────► web  (<product>-web, static bundle)
                    watchtower  (polls GHCR, label-scoped)
```

- **`proxy`** is an internal **same-origin router** — it reproduces the web app's dev proxy
  (`/api` + `/ws` → server, everything else → SPA) so the browser is same-origin. It
  **terminates no TLS**: the homelab gateway owns the public hostname and forwards plain
  HTTP. It also sets the security headers / CSP for the whole app in one place.
- It publishes on **loopback only** (`127.0.0.1:<port>`) — the gateway runs
  `network_mode: host`, so `localhost` reaches it but the LAN can't. Pick a free port (the
  dev box already uses 8090 for AdGuard, 8091 for someproduct, etc.).
- **`watchtower`** polls GHCR for new `:main` images and recreates only this stack's
  containers. It is **label-scoped** (`com.centurylinklabs.watchtower.scope=<product>`) so
  it ignores the rest of the box and isn't culled by other watchtowers, and **pinned** to
  `nickfedor/watchtower:1.17.2` (the maintained fork; containrrr is dead and negotiates a
  too-old Docker API). Auth its poll with `REPO_USER`/`REPO_PASS` (a `read:packages` PAT).

## Komodo GitOps

The stack is declared in `deploy/komodo/sync/*.toml`, wired into the same Komodo Core that
runs the homelab:

- **`stacks.toml`** — the `[[stack]]`: `server = "dev"`, `repo`, `branch = "main"`,
  `registry_provider/account = ghcr.io/charliethomson` (so Periphery can pull private
  images), `run_directory = "deploy/compose"`, `file_paths = ["prod.compose.yml"]`, and an
  `environment` block whose `[[VAR]]` references resolve against Komodo Variables.
- **`variables.toml`** — **non-secret values only**, and in practice often empty. Do **not**
  declare secrets here with `value = ""` — Komodo's sync diffs `value` literally and would
  wipe the UI-populated secret to empty on every run. Document the required secret names
  here as comments; create them in the UI.
- **`servers.toml`** — usually a comment explaining that the `dev` Server is **not**
  re-declared (homelab's own sync owns it; re-declaring causes a tug-of-war). `stacks.toml`
  references `server = "dev"` by name.

Deploy model: push to `main` → build workflows publish `:main` images → Watchtower (or a
manual **Deploy** in Komodo) rolls them out. Register the sync once: Komodo UI → Syncs →
New Sync → repo + branch `main` + resource path `deploy/komodo/sync` + Managed mode **off**.

## Secrets

- **Never in git.** Enter them as **secret Variables in the Komodo UI** (`Is Secret` ON);
  Komodo writes `.env` into the run directory at deploy time from the `environment` block.
- Common ones: `AUTH_ADMIN_KEY` (central auth TCP HMAC root — a *shared* secret; reuse, do
  not regenerate, or you rotate every app's key), `GHCR_TOKEN` (watchtower poll).
- `is_secret` only hides from UI/logs; values are stored at rest unencrypted, so the disk
  security of the Komodo volume is the real protection.

## Gateway ingress

Add the hostname block to the **homelab** repo's `gateway/Caddyfile`, then redeploy the
`gateway` stack:

```caddy
<product>.dev.thmsn.dev {
    tls {
        dns cloudflare {env.CF_API_TOKEN}
        resolvers 1.1.1.1 8.8.8.8
    }
    reverse_proxy localhost:<loopback-port>
}
```

TLS is via Cloudflare DNS-01 (no public A records; access over the tailnet).

## Telemetry

Point services at the homelab OTel collector: `OTLP_ENDPOINT=http://192.168.0.193:4317`
(the dev box LAN IP; otelcol fans out to Loki/Tempo for Grafana). Log ids/counts/timings,
never message content. Use the `dev.thmsn.<product>` identifier as the `service_name`.

Templates: [`templates/deploy/`](../templates/deploy/) (compose, Caddyfile, komodo sync TOML).

## Checklist

- [ ] `deploy/` has compose (`api`+`web`+`proxy`+`watchtower`) + internal Caddyfile + komodo sync.
- [ ] `proxy` terminates no TLS, publishes on loopback, sets security headers/CSP.
- [ ] Watchtower is label-scoped and pinned to the nickfedor fork.
- [ ] Komodo sync registered (Managed mode off); `dev` server not re-declared.
- [ ] Secrets only in the Komodo UI; `variables.toml` carries no `value=""` secrets.
- [ ] Gateway ingress block added to homelab `gateway/Caddyfile` for `<product>.dev.thmsn.dev`.
- [ ] Service exports telemetry to the homelab otelcol under its `dev.thmsn.<product>` name.
