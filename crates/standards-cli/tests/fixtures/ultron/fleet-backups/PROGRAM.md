# PROGRAM: fleet-backups

**Slug:** `fleet-backups` (the vision label) · **Primary repo:** `/Users/charlesthomson/git/stash`
· **Tracker:** vision, label `fleet-backups` across STASH, HOMELAB, VIS, TAV, AUTH, REG, DUN, ARGUS,
TRANS, DASH, WARD · **Planned:** 2026-10-13 by Jarvis

## SYSTEM CONTEXT

Before this program only vision backed up its own state (a shell sidecar,
`~/git/vision/deploy/compose/backup/vision-backup.sh`). The fleet standard
`~/git/standards/docs/backups.md` (written 2026-10-13, **uncommitted in ~/git/standards**: the user
commits it) makes it generic:

- **`stash-backup`**: a Rust lib + binary in the stash workspace (`server/backup`), shipped as
  `ghcr.io/charliethomson/stash-backup:main`. Every stateful stack runs it as a `backup` sidecar that
  mounts its data volume read-only and writes to `/mnt/truenas/Secure/Application Backups/<product>/`
  over NFS. Config is a committed `deploy/compose/backup.toml`.
- **The NAS directory is the only interface.** The sidecar writes snapshots, a file mirror,
  `.stash-backup/config.toml` (effective config) and `.stash-backup/runs.jsonl`. stash's server
  reads them (its `secure` root, fenced off from normal browsing) and owns the fleet view: per-app
  config, run history, audits, nightly restore drills, `stash_backup_*` metrics. The sidecar never
  talks to stash; stash never reaches into another stack.
- The drill and the sidecar's `verify` are the same library code. Drills unpack only the databases
  onto stash's local disk (`<data_dir>/drill`, never NFS: SQLite over NFS corrupts) and check the
  mirror in place on the NAS.

Invariants an implementer would otherwise guess at:
- Never copy a `.db` raw. Online backup API only. Journal mode from the file header; `immutable` only
  for a WAL db with no -wal/-shm, guarded by a fingerprint re-check.
- Fixed order: databases, then mirrors, then require_files, then publish (all DBs or none), then prune.
- No sentinel (`.<product>-backup-target`) ⇒ nothing written.
- Prune by name, never mtime. `.partial` + rename for every NAS write.
- One `libsqlite3-sys` in the stash graph: rusqlite pinned to sqlx's line.
- Keys that seal data never live under `/data` (standards security.md, updated in this session).

## FINDINGS / GOAL

Done = every stateful service in scope (vision, tavern, auth, registry, dungeon, argus, translate,
dash, warden, stash) shows in stash's Backups page with a passing audit and a passing nightly drill,
an alert covers failures fleet-wide, vision's legacy tree is gone, and tavern/dungeon/argus read
their sealing key from env.

Survey facts per repo are in `rollout.md`. Notable: registry, argus and translate use rollback
journal (not WAL); tavern's asset GC deletes files and writes in place; registry deletes version
dirs; auth connects as root and has no backup user; the game box mounts no NAS share; dev's
nas-guard covers `Files` only.

## LOCKED DECISIONS (do not relitigate)

- **LD-1** NAS only, vision's layout on NFS. No restic, no MinIO, no offsite tier (known gap).
- **LD-2** MariaDB: `mariadb-dump --single-transaction`, structural checks only in v1 (exit 0,
  `-- Dump completed` trailer, gzip round-trip). No load into a server. Client is MariaDB 11.x.
- **LD-3** `foreign_key_check` violations are recorded at backup time (run still publishes; stash
  flags it); `verify` (restore, drill) fails on them.
- **LD-4** Per-stack sidecar. The sidecar never talks to stash. The `backup` crate depends on no
  `stash-*` crate.
- **LD-5** Shipped only as the ghcr image, watchtower-managed, built by its own Woodpecker pipeline.
- **LD-6** Destination `/mnt/truenas/Secure/Application Backups/<product>`; sentinel
  `.<product>-backup-target`; layout per backups.md.
- **LD-7** Newly copied mirror files named by a require_files row are sha-checked before rename.
- **LD-8** `restore` chowns restored files to the owner of the database it moved aside.
- **LD-9** A failing `counts` query records null + warning, never fails the run. A failing
  require_files query fails the run.
- **LD-10** The per-run log line is raw stdout with a fixed prefix; `tracing` for detail. No OTLP
  from the sidecar (`network_mode: none`, except auth's which joins network `auth`).
- **LD-11** `verify`/`drill` take a `Pace` so the nightly sha256 sweep is throttled.
- **LD-12** The Backups section is web only.
- **LD-13** stash refuses to boot with `[backups]` on a root whose grant is `read`.
- **LD-14** `dev.thmsn.stash.backups` grant; access requires it **and** the root's grant;
  snapshot downloads allowed with those, logged at info.
- **LD-15** Drills: each product's newest daily snapshot, nightly.
- **LD-16** vision starts fresh under Secure; the old `Files/11 - Application Backups/vision` tree is
  left 30 days then deleted by hand. stash never reads the legacy layout.
- **LD-17** Rollout tasks live in each repo's own vision project. news and agentutil are out of this
  program. ARGUS project created for this program.
- **LD-18** nas-guard covering Secure on dev, and NAS mounts on the game box, are human-gated
  homelab tasks; they block the rollouts.
- **LD-19** Game-world backups (warden worlds, dash game servers) are out of scope.
- **LD-20** secret.key moves are follow-ons per repo. Until then `backup.toml` never lists the key,
  and the README says backups can't restore sealed values without it. Never generate a new key.
- **LD-21** verdaccio storage is excluded in v1.
- **LD-22** No runs.jsonl rotation in v1.
- **LD-23** vision is the pilot: every other rollout waits on VIS-71.

- **LD-24** Run order follows backups.md: sqlite, files, require_files, **then** mariadb (sidecar.md
  had mariadb before mirrors; the standard wins).
- **LD-25** The MariaDB password comes only from env `STASH_BACKUP_MARIADB_PASSWORD`, read directly
  (not through libconfig, which coerces types), `#[serde(skip)]`. Never from the file.
- **LD-26** Contract readers are lenient: unknown keys accepted, unknown enum words read as `Unknown`,
  because stash reads files from older and newer sidecars.

- **LD-27** The stdout lines keep the `<RFC3339Z>` prefix and the ok line carries `files_copied=`
  (sidecar.md and vision-backup.sh have them; backups.md's examples omit them). backups.md's example
  wants amending; the user commits ~/git/standards.
- **LD-28** A configured `[[mariadb]]` fails the run `reason=mariadb-dump` until the source exists, so
  no hourly ever publishes an incomplete set as ok. `RunRecord` carries optional `db`/`path`/`count`;
  `RequireFilesRun` carries optional `mismatched`.

- **LD-29** `stash-backup loop` runs once at boot, then at each `:<minute>` (`next_run` is strictly
  after now). The health state file is in `/tmp`, lost on every watchtower recreate; vision's script
  also ran at boot. Cost: one extra hourly per image roll.
- **LD-30** `restore` requires DIR (backups.md); `verify [DIR]` defaults to `data_dir`.

## PROGRAM CONSTRAINTS

- stash: run cargo from `server/`. Gate: `cargo fmt --all -- --check`, `cargo build --workspace
  --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`;
  route/schema change ⇒ regenerate `api/openapi.json` into a temp file then copy. Coverage floor 80%.
- Migration numbers: Ultron assigns (STASH-34 proposes `0007_backup_drill.sql`; check the next free).
- Tests are behaviour tests over tempdirs (standards testing.md); no mocks of the library under test.
- Rollout tasks: agent does the repo change; deploying, creating sentinels, and setting Komodo
  secrets are the user's.
- Nothing pushed without asking the user (fleet default is commit to main; ask before pushing).

## HUMAN GATES

- Pushing to origin.
- Every task labelled `human-gated` (host steps: nas-guard reinstall, NAS mounts, sentinels,
  deploys, Komodo secrets, the key migrations, deleting the legacy tree).
- Committing `~/git/standards` (the user has other uncommitted docs there).

## READ FIRST

- `~/git/standards/docs/backups.md` (the contract), `~/git/standards/docs/security.md` (keys)
- `sidecar.md` (canonical: crate, schemas, API, image, tests) beside this file
- `section.md` (stash server + web) beside this file
- `rollout.md` (per-repo facts with file:line) beside this file
- `~/git/vision/deploy/compose/backup/vision-backup.sh` (every edge case must survive the port)
- `~/git/stash/README.md`, `~/git/stash/DESIGN.md` (Roots, Collections, the `.stash/` invariant, Shape of the code)
- Task descriptions are also on disk in `tasks/*.md`; `tasks/refs.json` maps keys to refs.

## BACKLOG

All `todo`, label `fleet-backups`. Start: STASH-20, STASH-30, STASH-31 (agent), HOMELAB-17 and
HOMELAB-18 (agent part, then the user).

- **STASH-20** backup crate: skeleton, config and the on-NAS contract types (canonical; blocked by: none)
- **STASH-21** SQLite snapshot primitive (STASH-20)
- **STASH-22** NAS primitives (STASH-20)
- **STASH-23** additive mirrors + require_files (STASH-21, STASH-22)
- **STASH-24** `once`, runs.jsonl, log line (STASH-23)
- **STASH-25** verify / restore / drill API (STASH-23)
- **STASH-26** binary (STASH-24, STASH-25)
- **STASH-27** MariaDB source (STASH-24)
- **STASH-28** image + pipeline (STASH-26, STASH-27)
- **STASH-29** parity gate vs vision-backup.sh (STASH-28)
- **STASH-30** `[backups]` config + fence (none)
- **STASH-31** backups grant (none)
- **STASH-32** backups reader (STASH-20, STASH-30)
- **STASH-33** audits (STASH-32)
- **STASH-34** nightly drills (STASH-25, STASH-32)
- **STASH-35** metrics (STASH-33, STASH-34)
- **STASH-36** API + contract (STASH-31, STASH-33, STASH-34)
- **STASH-37** web section (STASH-36)
- **STASH-38** docs + deploy config (STASH-36, STASH-37)
- **STASH-39** stash's own sidecar [human-gated] (STASH-28, HOMELAB-17, VIS-71)
- **HOMELAB-17** nas-guard covers Secure [human-gated] (none)
- **HOMELAB-18** game box NAS mount + guard [human-gated] (none)
- **HOMELAB-19** Grafana alert (STASH-35)
- **VIS-71** vision switches to stash-backup [human-gated] (STASH-29, HOMELAB-17)
- **VIS-72** delete legacy tree after 30 days [human-gated] (VIS-71)
- **TAV-130 / AUTH-43 / REG-34 / DUN-16 / ARGUS-1 / TRANS-3** rollouts [human-gated] (STASH-28, HOMELAB-17, VIS-71)
- **DASH-6 / WARD-2** rollouts on the game box [human-gated] (STASH-28, HOMELAB-18, VIS-71)
- **TAV-131 / DUN-17 / ARGUS-2** sealing key to env [human-gated] (TAV-130 / DUN-16 / ARGUS-1)

## DISPATCH LOG

Started 2026-10-13 by Ultron. stash work runs in worktrees (isolation) and Ultron rebases onto
`main`; nothing is pushed without the user. homelab has the user's uncommitted monitoring edits:
agents there stage only their own paths. STASH-31 waits on STASH-30 in practice (backups_for needs
the `[backups]` config), though vision has no relation.

| task | state | note |
|---|---|---|
| STASH-20 | dispatched | B1, worktree |
| STASH-30 | dispatched | X1, worktree |
| STASH-31 | queued | after STASH-30 lands |
| HOMELAB-17 | dispatched | agent part only, human-gated remainder |
| HOMELAB-18 | dispatched | agent part only (same agent as HOMELAB-17), human-gated remainder |
| HOMELAB-19 | queued | blocked on STASH-35 |
| STASH-21 | queued | |
| STASH-22 | queued | |
| VIS-71 | queued | human-gated |
| HOMELAB-17 | blocked | agent part landed e2fe997 (unpushed); human-gated: push, install-nas-guard on dev, mkdir Application Backups |
| HOMELAB-18 | blocked | agent part landed 22a49bf (unpushed); human-gated: fstab mount + nas-guard on game |
| STASH-20 | landed | 33ffd02 on main (unpushed); verified |
| STASH-40 | dispatched | filed by Ultron: clippy 1.99 assert_is_empty in pre-existing tests; worktree |
| STASH-21 | dispatched | B2, worktree |
| STASH-22 | dispatched | B3, worktree |
| STASH-30 | landed | 07bf110 on main (unpushed); verified; [backups] enabled in deploy stash.toml |
| STASH-40 | landed | 0fdbef4 (from the STASH-30 worker); dedicated worker stopped |
| STASH-31 | dispatched | worktree |
| STASH-21 | landed | d40e7a8 on main (unpushed); verified |
| STASH-31 | landed | 4a59fe6 on main (unpushed); verified |
| STASH-22 | landed | 5a9d33a on main (unpushed); verified |
| STASH-23 | dispatched | worktree |
| STASH-32 | dispatched | worktree |
| HOMELAB-17 | landed | human part done on dev; whisparr restart proved it; done |
| HOMELAB-18 | blocked | human-gated: game set up, only the remount test left |
| STASH-23 | landed | 09ee9dc on main (unpushed); verified |
| STASH-24 | dispatched | worktree |
| STASH-25 | dispatched | worktree |
| STASH-32 | landed | f4b823a on main (unpushed); verified |
| STASH-33 | dispatched | worktree |
| STASH-24 | landed | d6602d6 on main (unpushed); verified |
| STASH-27 | dispatched | worktree |
| STASH-33 | landed | 15b21c8 on main (unpushed); verified |
| STASH-25 | dispatched | resumed after the disk-full stop |
| STASH-27 | dispatched | resumed after the disk-full stop |
| STASH-25 | landed | 5b42f6c on main (unpushed); verified |
| STASH-26 | dispatched | worktree |
| STASH-34 | dispatched | worktree |
| STASH-27 | landed | 00ea494 on main (unpushed); verified |
| STASH-42 | dispatched | follow-on from STASH-32; one worker with STASH-43 |
| STASH-43 | dispatched | follow-on from STASH-27; blocks AUTH-43 |
| STASH-42 | landed | b3348cb on main (unpushed); verified |
| STASH-43 | landed | d6d9d4b on main (unpushed); verified |
| STASH-26 | landed | 36ef80f on main (unpushed); verified |
| STASH-28 | dispatched | worktree |
| STASH-34 | landed | c13dd49 on main (unpushed); verified |
| STASH-35 | dispatched | worktree |
| STASH-36 | dispatched | worktree |
| STASH-28 | landed | e12e8fb on main (unpushed); image verified locally by the worker; pipeline untested until push |
| STASH-29 | blocked | needs a copy of vision prod data on the mac; asked the user how (2026-10-13) |
| STASH-35 | landed | 84b2a2f on main (unpushed); verified |
| HOMELAB-19 | dispatched | agent writes the rule file; unpushed |
| HOMELAB-19 | blocked | e7cb5ea on homelab main (unpushed); human-gated: push, Komodo sync, firing test |
| STASH-36 | landed | 970d1dd on main (unpushed); verified; conflicts with STASH-35 resolved by Ultron |
| STASH-37 | dispatched | worktree |
| STASH-37 | landed | 480a047 on main (unpushed); web checks green; real-backend browser check pending (Ultron) |
| STASH-38 | dispatched | worktree |
| STASH-37 | landed | 480a047 + ddd8bb4 (UTC date fix); browser-verified against a real stash-api |
| STASH-38 | landed | 4042634 on main (unpushed) |

Preview harness (uncommitted, for re-checking the web UI against a real stash-api with fake auth): ~/.local/state/thmsn/ultron/fleet-backups/preview-harness/ (drop preview_harness.rs into server/api/src, apply main-rs.diff, run `PREVIEW_ADDR=127.0.0.1:8081 cargo test -p stash-api --bin stash-api preview_harness -- --ignored --nocapture`; accounts archivist-account / insider-account, password hunter2; launch.json config web-preview-8081).

## RESUME POINT (2026-10-13, end of agent-workable stash side)

Landed (all unpushed on stash main 4042634, 23 commits ahead of origin): STASH-20..28, 30..38, 40, 42, 43.
Full gate green on 4042634 (fmt, clippy -D warnings, tests, contract, web codegen/typecheck). Coverage
(tarpaulin, 80% floor) NOT measured locally; CI will tell on push.
homelab main: e2fe997, 22a49bf, a5d00dd pushed; e7cb5ea, 4de908e (HOMELAB-20), bc95b54 unpushed.

Waiting on the user:
- push stash main (triggers image.build.yml and the new backup-image.build.yml; first real run of the latter)
- push homelab + Komodo sync (HOMELAB-19 alert), then the firing test once a product backs up
- STASH-29 parity: how to get vision data (copy legacy snapshot to the mac, or run on dev after push)
- HOMELAB-18 game remount test
- ~/git/standards backups.md amendments (LD-27 log prefix + files_copied; LD-30 verify [DIR]) — user commits standards
Next agent work after that: VIS-71 (human-gated: agent writes vision's backup.toml + compose, user deploys), then the rollouts.
| PUSH | landed | stash main 4042634 and homelab main bc95b54 pushed 2026-10-13 at the user's request |
| STASH-44 | landed | 205d87a pushed; pipeline 26 green |
| STASH-45 | landed | 1d06af0 pushed |
