# Spec: stash's Backups section (server + web)

Source: a spec agent's read of stash @ main, reconciled by Jarvis 2026-10-13. Consumes the contract
in `sidecar.md` (canonical) and `~/git/standards/docs/backups.md`. Web only (LD-12).

## Config and the fence

```toml
# deploy/compose/stash.toml
[backups]
root = "secure"
path = "Application Backups"
```

Modelled on `[collections]` (`server/api/src/config.rs:65-67,165-174`; DESIGN.md "Where they live").
Scalars under `STASH_*` (`config.rs:7-9` style): `backup_drill_hour_utc` (4),
`backup_audit_interval_secs` (300), `backup_scratch_dir` (`<data_dir>/drill`),
`backup_drill_bytes_per_sec` (32 MiB/s, like dupes).

Boot validation beside `mount_roots` (`server/api/src/main.rs:279-285`): refuse unknown root,
overlap with the collections path, and a root whose grant is `read` (LD-13). Don't require or create
the directory: stash never writes there.

**Fence.** The tree must vanish from normal browsing: `.stash-backup` doesn't match `OWN_NAMES`
(`storage/src/lib.rs:60`); vision writes the same gzip as hourly and daily so dupes would hash GBs
nightly (`engine/src/dupes.rs`); `secure` is writable, so Files routes could delete snapshots
(`state.rs:249-258`); search and thumbnails would leak names past the `backups` grant.
- `MountedRoot.fenced: Vec<String>` set in `mount_roots`.
- `MountedRoot::resolve` (`lib.rs:113-115`) and `resolve_for_create` (`path.rs:127`) return NotFound
  inside a fence: stat, download, thumb, poster, move, copy, delete in one door (DESIGN 608-620).
- `list` (`lib.rs:217`) and `walk::scan` (`walk.rs:78-88`) skip it; search + dupes follow from the index.
- `MountedRoot::unfenced()` for the backups reader (`stat`, `open_range` at `lib.rs:279,301`).
- Existing index rows go on the next complete crawl sweep; check digest rows go too.
- DESIGN.md paragraph: why this isn't the dupe-ack "nothing is excluded from the scan" rule (404-410).

## Grant

`#[grant(display_name = "Backups", description = "View application backups")] Backups` in
`server/api/src/auth.rs:57-74` (+ doc table `:8-15`) ⇒ `dev.thmsn.stash.backups`. Not in
`Grants::from_leaf` (`state.rs:363-375`). `AppState::backups_for(user) -> Result<&backups::Place>`
returns `NotFound{entity:"backups"}` unless configured and the user holds `backups` **and** the
root's grant (`root_for`, like `collections_for` `state.rs:166-170`; LD-14). Update fake auth
(`auth.rs:574-604`).

## Engine: audits and drills

`stash_engine::backups::run`, spawned in `main.rs` (~200-215) beside the janitor/indexers, shutdown
via `select!` like `janitor.rs:51-72`. Engine reports through callbacks (`engine/src/lib.rs:22-26`).
- **Audit pass** every `backup_audit_interval_secs`: read every product, compute audits, hand a
  `Report` to a callback (metrics) and a `Handle` (cached mirror sizes for the API).
- **Drill** nightly at `drill_hour` (LD-15): each product's newest `db/daily/*`, one at a time,
  `spawn_blocking(stash_backup::drill(..., Pace))`. Due times in a new table `backup_drill`
  (migration `0007_backup_drill.sql`, Ultron confirms the number), reboot logic like
  `crawl::first_due` (`engine/src/crawl.rs:265-283`). Skip when the backups root is degraded.
  "Drill now" nudges like `dupes::Handle::nudge`.
- **Scratch** `<data_dir>/drill/<product>/` on the `stash-data` volume (local disk, never NFS).
  Wipe at boot and after each drill. Before unpacking, `statvfs` ≥ 2× gzip ISIZE else record
  `skipped reason=no-scratch` (counts as an audit failure, `check=drill`).

Audits (pure function over the reader's output; table tests):
- freshness: newest ok run vs that product's `max_age`
- retention: hourly/daily counts above config ⇒ pruning failing; missing daily in last N days ⇒ warn
- partial: a `.partial` older than 1h
- sentinel: missing
- trend: newest `bytes` or any `counts` value < 50% of the median of the last 24 ok runs
- integrity: `foreign_key_violations > 0` in the latest run (LD-3)
- config: effective config unreadable / schema unknown
- drill: last drill failed, skipped, or older than 2 days

## Metrics

`BackupMetrics` in `server/api/src/metrics.rs` like `DupeMetrics` (`:150-186`), refreshed in
`render` (`:375-392`). Labels: `product`, `db`, fixed `check`/`tier`/`outcome`.

| series | type | labels |
|---|---|---|
| `stash_backup_last_success_timestamp_seconds` | gauge | product |
| `stash_backup_last_run_timestamp_seconds` | gauge | product |
| `stash_backup_snapshots` | gauge | product, tier |
| `stash_backup_snapshot_bytes` | gauge | product, db |
| `stash_backup_audit_failures` | gauge 0/1 | product, check |
| `stash_backup_last_drill_timestamp_seconds` | gauge | product |
| `stash_backup_last_drill_success_timestamp_seconds` | gauge | product |
| `stash_backup_drills_total` | counter | product, outcome |
| `stash_backup_drill_duration_seconds` | gauge | product |

## API (`routes/backups.rs`, `Tag::Backups`, all through `backups_for`)

| route | operationId | returns |
|---|---|---|
| `GET /api/v1/backups` | listBackups | `[ProductSummary{product, status ok|warn|fail, last_success, last_run_outcome, last_drill{at,outcome}, failing:[check]}]` |
| `GET /api/v1/backups/:product` | getBackup | `ProductView{config_toml, config, audits:[Check{check,status,detail}], snapshots{hourly,daily:[{name,path,bytes,taken_at}]}, mirrors:[{name,files,bytes}], drills:[DrillView], sentinel}` |
| `GET /api/v1/backups/:product/runs?limit&before` | listBackupRuns | runs newest first, tail read, torn line tolerated |
| `GET /api/v1/backups/:product/snapshot?path=` | downloadSnapshot | Download 200/206/304/416, only under `db/` |
| `POST /api/v1/backups/:product/drill` (`require_origin`) | drillBackup | 202 |

Extract `download_file`'s If-None-Match/If-Range/Range logic (`routes.rs:215-270`) into a shared
`serve(stat, &Request) -> Download`. Snapshot downloads are allowed for the `backups` grant (LD-14)
and log `info` with user + product. `:product` must be a directory name in the backups path; `path`
via `path::resolve` under `<product>/`. Register `route_name` entries (`app.rs:158`, test `:727`);
regen `api/openapi.json` (drift check `.woodpecker/rust.ci.yml:103-104`; client drift `web.ci.yml:52`).

## Web

No router; `view` state in `App.tsx` (~74-80, switch 352-357), `View` in `components/Sidebar.tsx:4`,
tools at `Sidebar.tsx:~84-110`. Feature detection by 404 probe (`App.tsx:163-172`). Keys
`api/keys.ts`, types `api/client.ts`. Model on `components/Trash.tsx`. Follow the repo (it doesn't
use Zustand or `@thmsn/ui` yet).
- `View |= 'backups'`; probe `qk.backups` → `listBackups`, 404 ⇒ no tool.
- `components/Backups.tsx`: row per product: status, last success ("2h ago"), last drill, failing checks.
- `components/BackupProduct.tsx`: Checks, Snapshots (`<a href>` downloads, cookie-authed per
  `routes.rs:207-214`), Runs, Drills + "Drill now", Config (`<pre>`).
- Copy: "Backups", "ok / late / failing", "Drill now", "No backups yet.", "Never drilled."
