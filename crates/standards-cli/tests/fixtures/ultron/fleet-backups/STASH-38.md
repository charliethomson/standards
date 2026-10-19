# STASH-38 journal — stash docs and deploy config for backups

PLAN
- [x] 0. rebase worktree onto main (480a047)
- [x] 1. read context
- [x] 2. vision comment: start (PLAN) — posted D4V0FAVPAWZ
- [x] 3. DESIGN.md
- [x] 4. auth.rs doc grant table: already complete, no edit (so no cargo gates needed)
- [x] 5. AGENTS.md
- [x] 6. README (+ server/README.md stale STASH-35 note)
- [x] 7. SHORTCUTS.md
- [x] 8. deploy/compose/stash.toml
- [x] 9. verify help output matches README; commit — c9e15a9
- [x] 10. vision comment: finish (evidence) — posted. TASK DONE.

STATE
- step 10. Everything committed (c9e15a9 on worktree-agent-ace0d82b581b48d81), nothing half-applied.
- Findings to report: homelab backups.yml alert description lists partial/trend as failing
  causes but the audit only warns on them (outside repo, not changed).

TOUCHED
- AGENTS.md, DESIGN.md, README.md, SHORTCUTS.md, deploy/compose/stash.toml, server/README.md
- commit c9e15a9 `docs: backups (STASH-38)`

VERIFY
- cd server && cargo run -q -p stash-backup -- help  -> byte-identical to README block (checked)
- cargo run -q -p stash-backup -- restore latest -> "restore needs a DIR", exit 2 (checked)
