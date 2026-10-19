# HOMELAB cutover prep (HOMELAB-9, -10 vhost half, -12 branch)

## PLAN
1. HOMELAB-9: monitoring/stash.dashboard.json, provisioned like the others, config_files tracked; validate JSON + metric names vs stash metrics.rs @4285b06; commit+push main.
2. HOMELAB-10 (vhost only): gateway/Caddyfile `stash.dev.thmsn.dev, fs.thmsn.dev {`, inert comment; validate caddy; check gateway config_files/post_deploy; commit+push main; comment, leave todo.
3. HOMELAB-12: branch stash-cutover/remove-fs; remove fs/ + refs in stated order; greps; push branch only; comment; leave in_progress.

## STATE
- HOMELAB-9 DONE (agent side): a79fbc8 pushed to main; commented; left in_progress pending human sync + Grafana check.
- HOMELAB-10 vhost DONE: 1cd20f6 pushed to main; commented; state todo (DNS half is human).
- HOMELAB-12 DONE (agent side): branch stash-cutover/remove-fs pushed (9 commits 2d201b7..dcb540b), NOT merged; commented; in_progress. Local checkout back on main, clean.
- ALL THREE TASKS COMPLETE for the agent. Remaining = human: sync (H9), AdGuard flip (H10), H11, then merge branch (H12).

## TOUCHED
- homelab main a79fbc8: monitoring/stash.dashboard.json (new), monitoring/docker-compose.yml (mount), komodo/sync/stacks.toml (config_files)
- homelab main 1cd20f6: gateway/Caddyfile (site list + comment)
- homelab branch stash-cutover/remove-fs: fs/ removed; prometheus.yml; _scripts/migrate-promtail-to-alloy.sh; komodo/sync/{stacks,servers,variables}.toml; dns/AdGuardHome.yaml.template; komodo/scripts/{backup-keys,import-env-secrets}.sh; komodo/periphery/.env.example; docs (README, CLAUDE, HOSTS, RUNBOOK, PROX-MONITORING, dns/README, komodo/README, gateway/README, monitoring/README)
- generator + checker: scratchpad gen_stash_dashboard.py, check_metrics.py

## VERIFY
- H9: json.tool OK; check_metrics OK 0 unknown, all 13 families used; live prom: all 33 exprs success; thumbs/search/uploads/5xx empty (no activity)
- H10: gateway image built from gateway/Dockerfile; caddy validate before+after 'Valid configuration'; adapt diff: only stash route host matcher changed, handler identical, no Host rewrite. gateway config_files=[Caddyfile], post_deploy restart caddy OK.
- H12: git grep 192.168.0.62 empty (worktree too), FS_HOST_IP empty; copyparty only historical + public-source tile (flagged); promtool syntax OK; toml parses; bash -n OK. Untracked fs/.env, fs/live/* left on disk (names only).
