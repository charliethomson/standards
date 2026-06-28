# Archetype: Orchestration

Infrastructure-as-code for the homelab itself — the repo that *deploys* the products
rather than being one. There is essentially one of these (**homelab**), but the
conventions are worth fixing so it stays coherent.

Reference repo: **homelab**.

## Repo shape

One top-level directory per deployable stack; each directory is a complete, independently
deployable unit. A central control plane (Komodo) reconciles them from git.

```
homelab/
├── <stack>/                one dir per stack (gateway, dns, media, registry, monitoring…)
│   ├── docker-compose.yml
│   ├── .env.example        non-secret template, committed
│   └── README.md
├── komodo/
│   ├── periphery/          the per-host agent compose
│   └── sync/               declarative source of truth
│       ├── servers.toml    hosts running Periphery
│       ├── stacks.toml     every stack + its run directory + pre_deploy hooks
│       ├── variables.toml  non-secret shared vars (host IPs, regions); secrets live in UI
│       └── procedures.toml manual/one-shot procedures
├── docs/                   HOSTS.md (IP inventory), RUNBOOK.md (recovery)
├── CLAUDE.md               deployment model + gotchas
└── standards/              this submodule
```

## How it works

- **Komodo + Periphery GitOps.** Git `main` + the `komodo/sync/*.toml` are the source of
  truth. Komodo Core reconciles; stateless Periphery agents on each host pull and run
  `docker compose`. You deploy by pushing to `main` and executing the sync.
- **Caddy ingress** (custom image with the Cloudflare DNS plugin) terminates TLS via
  DNS-01; services sit behind it on the internal network.
- **Watchtower** auto-updates containers from `:main`/`:latest` images, scoped by label.

## Standards that apply

| Standard | How it applies here |
|---|---|
| [Deployment](../deployment.md) | This is the canonical implementation of it — Komodo + Periphery + Caddy + Watchtower. |
| [Identifiers](../identifiers.md) | DNS naming `<service>.<host>.thmsn.dev`; registry source `dev.thmsn.apps`. Distinct from product identifiers. |
| [Workflow](../workflow.md) | Commit to `main`; reconcile. |
| Secrets split | **The defining rule** — see below. |
| Versioning / Build info / Branding / Testing / Platform UX | **N/A** (it deploys versioned artifacts; it isn't one). |

## Secrets & config split (the defining rule)

- **In git:** only non-secret config — `.env.example` per stack, and non-secret shared
  values in `komodo/sync/variables.toml` (host IPs, region names). Host IPs live in exactly
  one place and are consumed by DNS, Caddy, and Prometheus via `envsubst`.
- **Never in git:** secrets. They are entered as **secret Variables in the Komodo UI** and
  interpolated at deploy time. `.gitignore` covers `.env`, `public/*`, `.DS_Store`.
- **Config rendering** happens at deploy time via `pre_deploy` `envsubst` sidecars, not by
  committing rendered files.

## Naming

`<service>.<host>.thmsn.dev` — e.g. `sonarr.dev.thmsn.dev`, `fs.thmsn.dev`,
`agentutil.dev.thmsn.dev`. Off-network resolution is intentionally NXDOMAIN (no public A
records); access is via the tailnet.

## Checklist

- [ ] One directory per stack; each independently deployable with its own compose + README.
- [ ] `komodo/sync/*.toml` is the declarative source of truth, committed.
- [ ] Secrets only in the Komodo UI; git holds `.env.example` + non-secret `variables.toml`.
- [ ] Host IPs defined once and referenced via `envsubst`.
- [ ] Ingress via Caddy (DNS-01 TLS); auto-update via scoped Watchtower.
- [ ] `docs/HOSTS.md` + `docs/RUNBOOK.md` kept current; gotchas in `CLAUDE.md`.
