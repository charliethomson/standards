# HOMELAB-19 — Grafana: alert on stash_backup audit failures and stale drills

PLAN        — [x] done  [ ] not yet
- [x] 1. Write this journal
- [x] 2. Read conventions: alerting files, contact-points, infra.yml target-down rule (read only), prometheus job for stash, stash.dashboard.json, grafana-dashboards.md, STASH-35 comments
- [x] 3. Comment on HOMELAB-19: start + plan
- [x] 4. Write monitoring/grafana/provisioning/alerting/backups.yml
- [x] 5. Add backups row/panels to monitoring/stash.dashboard.json
- [x] 6. Add grafana/provisioning/alerting/backups.yml to komodo/sync/stacks.toml monitoring config_files (dir mount ./grafana:/etc/grafana covers it; config_files is what triggers deploy)
- [x] 7. Validate: python yaml/json load; any repo lint
- [x] 8. Commit only my paths: "monitoring: alert on stash backup audits and stale drills (HOMELAB-19)"
- [x] 9. Comment on HOMELAB-19: finish (SHA, validation, human steps)

STATE       — DONE. Commit e7cb5ea on homelab main, unpushed; finish comment posted.
TOUCHED     — COMMITTED e7cb5ea: monitoring/grafana/provisioning/alerting/backups.yml (new, uncommitted); komodo/sync/stacks.toml (+1 config_files line, uncommitted); monitoring/stash.dashboard.json (Backups row, uncommitted). DO NOT touch user's dirty files: monitoring/README.md, monitoring/grafana/provisioning/alerting/infra.yml, _scripts/bootstrap-host-telemetry.sh
VERIFY      — python yaml load all alerting files (57 unique uids, conditions resolve, key set == vision.yml), json load dashboard (ids unique), tomllib stacks.toml has backups.yml, go text/template exec of value+summary annotations: ALL OK. promtool n/a (Grafana-managed rules).
