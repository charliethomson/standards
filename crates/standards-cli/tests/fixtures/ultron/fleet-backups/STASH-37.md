# STASH-37 — stash web: Backups section (implementer journal)

Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-afe83e1a82099455b
Branch: worktree-agent-afe83e1a82099455b (rebased onto main 970d1dd)

PLAN
- [x] 0. Rebase on main (970d1dd). Write this journal.
- [x] 1. Comment start on STASH-37 with plan (comment HT7QVE7MA1E).
- [x] 2. Read apps/web: App.tsx, Sidebar.tsx, Trash.tsx, api/keys.ts, api/client.ts, schema.d.ts (backups types), mutations.ts, vite.config.ts, web.ci.yml.
- [x] 3. keys.ts: qk.backups / backup(product) / backupRuns(product). client.ts types + snapshotUrl. View |= 'backups'; Sidebar tool; App probe + switch.
- [x] 4. components/Backups.tsx (list rows: status ok/late/failing, last success relative, last drill, failing checks).
- [x] 5. components/BackupProduct.tsx (Checks, Snapshots <a href>, Runs paged, Drills + Drill now POST, Config <pre>).
- [x] 6. Phone width (Tailwind classes, as the rest of the app).
- [x] 7. Run web.ci.yml checks (codegen:check, typecheck, build; no lint script exists).
- [x] 8. Visual check via dev server if backend reachable.
- [x] 9. Commit "web: the Backups section (STASH-37)"; comment finish on STASH-37.

STATE
- step 3 done (also ago() in lib/format.ts, drillBackup in api/mutations.ts, snapshotUrl + Backup* types in api/client.ts).
- step 4 done: components/Backups.tsx written (exports Backups, useBackups, BackupsError, Status, Offline).
- step 5 done: components/BackupProduct.tsx written; BackupsError/Status/Offline moved into it (no import cycle). Phone width handled inline with flex-wrap/basis-full (step 6).
- step 7 done: codegen:check exit 0, typecheck clean, build ok (94 modules).
- step 8 done: verified against scratch mock (list, product, drill now w/ Origin header, Older paging 20→40, phone 375px no h-scroll, 404 hides tool). Fixes from it: sm:w-40 name column, line/lines plural. README.md line added. Checks re-run green.
- step 9 done: committed 480a047; finish comment W7TQ471AYQE posted. TASK COMPLETE.
- (old) step 8: visual check. Plan: no local stash-api (needs auth socket etc.), so run vite dev with a scratch mock API (s37-mock.mjs in scratchpad, NOT in repo) on STASH_SERVER and view via Browser pane.

TOUCHED
- apps/web/src/api/{keys,client,mutations}.ts, src/lib/format.ts, src/components/Sidebar.tsx, src/App.tsx, src/components/{Backups,BackupProduct}.tsx, README.md — all in commit 480a047

VERIFY
- cd apps/web && npm run codegen:check && npm run typecheck && npm run build — last run: all pass (codegen:check exit 0; typecheck clean; build ok)
