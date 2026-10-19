# Backups

## Rule

**Every service with state backs that state up to the NAS with the `stash-backup` sidecar**,
a container in the service's own stack that mounts its data volume read-only. What gets
backed up and how it's checked lives in a committed `backup.toml`. **stash** reads
everything the sidecars write and owns the fleet-wide view: per-app config, run history,
freshness and retention audits, and scheduled restore drills.

> A backup that has never been restored is a hope. The drill is part of the backup.

`stash-backup` grew out of a per-service shell sidecar, made generic, in Rust, with its edge
cases as tests. Every design decision below was paid for there first.

## What this covers

**Service state**: the SQLite files and file trees under a service's `/data`, and a MariaDB
database where a service runs one. One sidecar per stack, one product directory on the NAS.

**Not covered: backups as a product feature.** A product that backs up its users' data (game
worlds, uploaded projects) is doing something for its users, with its own retention and UI.
That follows one rule from here: **never on the same disk as the source.** The product's own
database is service state and does get a sidecar.

## The pieces

```
 stack (any host with the NAS mounted)             NAS: Secure/Application Backups/
┌──────────────────────────────────┐              ┌──────────────────────────────────┐
│ api ── rw ──► <product>-data     │              │ <product>/                       │
│                    │             │              │   .<product>-backup-target       │
│ stash-backup ◄─ ro ┘             │──── NFS ────►│   .stash-backup/config.toml      │
│   backup.toml (committed, ro)    │              │   .stash-backup/runs.jsonl       │
└──────────────────────────────────┘              │   db/hourly  db/daily  <mirrors> │
                                                  └───────────────┬──────────────────┘
                                                                  │ reads (own root)
                                                  stash: Backups section, audits,
                                                  restore drills, stash_backup_* metrics
```

- **`stash-backup`** is `ghcr.io/charliethomson/stash-backup:main`, built from the stash
  repo's workspace (`dev.thmsn.stash.backup`). Watchtower-managed like any first-party image.
  Subcommands: `loop`, `once`, `health`, `verify DIR`, `restore SNAPSHOT DIR`.
- **The sidecar never talks to stash, and stash never reaches into a stack.** The NAS
  directory is the whole interface. stash being down costs the view, never a backup.
- The backup, verify and drill logic is one crate in the stash workspace, linked by both
  binaries, so a drill in stash checks exactly what `verify` checks at restore time.

## What a backup is

The order is fixed in code. The toml lists sources; it does not order them.

1. **Each SQLite database, through SQLite's online backup API**, opened read-only. **Never copy
   a `.db` raw.** In WAL mode the `.db` file alone is not the data (committed transactions can
   sit in the `-wal` for a long time); in rollback-journal mode a raw copy can catch a
   half-applied transaction. The backup API is right for both. The copy is made on the
   container's local disk, checked, gzipped, and only then written to the NAS. A failed
   `integrity_check` fails the run. `foreign_key_check` violations are recorded in
   `runs.jsonl` and flagged by stash, but don't block the snapshot: a service with old
   violations would otherwise lose every backup. `verify` (restores and drills) fails on them. SQLite over NFS corrupts; a finished gzip over NFS is a
   file.
   - **WAL databases only:** a cleanly stopped service leaves no `-shm`, and a read-only mount
     can't create one, so the normal open fails. With no `-wal` and no `-shm`, no writer is
     attached: the file is read as `immutable`, and the copy is discarded if a `-wal`/`-shm`
     appeared or the file's size or mtime changed by the end. The journal mode comes from the
     file header (bytes 18-19 are `2` in WAL mode), never from the absence of a `-shm`: a
     rollback-journal database never has one, running or not, and reading a live one as
     `immutable` is exactly the raw copy this step exists to avoid.
2. **File trees, after the databases**, into an **additive mirror**: a file already in the
   mirror is never overwritten. Services write a file before committing the row that names
   it and don't rewrite files in place, so every row in the snapshot has its file by the
   time this step runs, and a later corruption of a source file can't propagate into the
   mirror. A service that rewrites files in place or deletes them (an orphan GC, deleting
   old versions, a manifest updated in place) needs a different source kind.
   Raise it here before working around it.
3. **`require_files` checks**: every `(file, sha256)` row a configured query returns must be
   in the mirror, and a file copied in this run must match its sha256 before it is renamed
   into place, so a corrupt source file never enters the mirror. A failure fails the run.
   Paths a query returns are one relative path under the mirror; `..`, absolute paths and NUL
   fail with `reason=unsafe-path`.
4. **MariaDB**: `mariadb-dump --single-transaction`, gzipped, as
   `db/hourly/<database>-<stamp>.sql.gz`. Checked structurally: the dump exits 0, ends with
   `-- Dump completed`, and the gzip round-trips. Loading it into a real server is not done
   yet; the client in the image must match the server's MariaDB major version.

Every write to the NAS goes to `<name>.partial` and is renamed into place, so a partial file
never sits under a final name. The databases of one run publish together or not at all.

## Where it goes

```
/mnt/truenas/Secure/Application Backups/<product>/
  .<product>-backup-target       sentinel, created by hand once
  .stash-backup/
    config.toml                  effective config of the last run (file + env, resolved)
    runs.jsonl                   one JSON object per run
  db/hourly/<db>-YYYYMMDDTHHMMSSZ.db.gz    (.sql.gz for MariaDB)
  db/daily/<db>-YYYYMMDD.db.gz             first good snapshot of each UTC day
  files/<name>/...               additive mirror per [[files]] source
  last-success                   for humans
```

- **`Secure`, not `Files`.** A snapshot is a whole database: sealed credentials, password
  hashes, everything. `Files` is the broadly-readable share stash serves under its `read`
  grant.
- **No sentinel, no write.** An unmounted NFS path is an empty directory on the host's root
  disk, and a backup that lands there shares a disk with the data. The sidecar refuses to
  write unless `.<product>-backup-target` exists, and logs `reason=no-target`. Create it once,
  on the host, as your own user (root may be squashed over NFS):

  ```sh
  mkdir -p '/mnt/truenas/Secure/Application Backups/<product>'
  touch '/mnt/truenas/Secure/Application Backups/<product>/.<product>-backup-target'
  ```
- **Prune by name, never by mtime.** The names sort chronologically. The NAS clock and a
  copied file's mtime are not evidence of age.
- **The sidecar runs as `3000:3000`**, the NAS tree's owner (homelab `media/README.md`). A
  service whose `/data` isn't world-readable needs its files readable by that uid.
- **NAS only, for now.** Every copy, including Komodo's dumps, lives on the one NAS. An
  offsite tier is a known gap, not an oversight; adding one is a change to this doc.

## Configuration

`deploy/compose/backup.toml`, committed, mounted at `/etc/stash-backup/config.toml` and
loaded with `libconfig`'s `Loader::path` ([configuration.md](configuration.md)). Env under
`STASH_BACKUP_` overrides **top-level** scalars only (`STASH_BACKUP_PRODUCT`,
`STASH_BACKUP_MARIADB_PASSWORD`); libconfig's env layer is flat, so `[schedule]` and
`[retention]` are file-only. Secrets arrive only through env, and the effective config written
to the NAS never contains them. A config with no `product` fails with `reason=config`: a
missing mount must not look like an empty, valid config.
Defaults live in code; a typical service's file is four lines:

```toml
product = "someproduct"

[[sqlite]]
path = "/data/someproduct.db"
```

The full shape, for a service that stores attachments as files named by row id:

```toml
product = "someproduct"          # sentinel name, log prefix, NAS directory

[schedule]                       # defaults shown
minute = 7                       # hourly, at :07
max_age = "2h10m"                # `health` fails past this; stash audits against it

[retention]                      # defaults shown
hourly = 48
daily = 30

[[sqlite]]
path = "/data/someproduct.db"    # name defaults to the file stem ("someproduct")
counts = ["item", "attachment"]  # logged per run; stash trends them

[[sqlite.require_files]]
mirror = "attachments"
query = """
SELECT substr(h,1,8)||'-'||substr(h,9,4)||'-'||substr(h,13,4)||'-'||substr(h,17,4)||'-'||substr(h,21,12), sha256
FROM (SELECT lower(hex(id)) AS h, sha256 FROM attachment)
"""

[[files]]
name = "attachments"
path = "/data/attachments"
```

```toml
product = "someproduct"

[[mariadb]]
host = "maria"                   # the compose service name
database = "someproduct"
user = "backup"                  # password: STASH_BACKUP_MARIADB_PASSWORD, a Komodo secret
```

`integrity_check` and `foreign_key_check` always run; they are not config. A `counts` query
that fails (a renamed table) records `null` and a warning; it doesn't fail the run. **Nothing under
`/data` is backed up unless it is listed**, which is what keeps a stray key file out of the
snapshots (next section).

## Keys stay out of `/data`

A key that seals data in the database must not live in the volume that gets snapshotted. If
it does, every snapshot carries the key beside the ciphertext it opens, and the sealing
defends against nothing that has the backup. Keys arrive as env from a Komodo secret
Variable ([deployment.md](deployment.md), [security.md](security.md)), e.g.
`SOMEPRODUCT_SECRET_KEY`.

A service that still generates a `secret.key` in its data dir ([security.md](security.md) used
to prescribe it) must not list it in `backup.toml` until it moves (it isn't listed unless you
list it), and **a lost key is then unrecoverable from backups**: write that down in the repo's
README beside the key.

## The compose block

```yaml
  # Hourly snapshots of {{PRODUCT}}-data onto the NAS (standards/docs/backups.md).
  # Config: ./backup.toml. Restore: SHORTCUTS.md.
  backup:
    image: ghcr.io/charliethomson/stash-backup:main
    restart: unless-stopped
    user: "3000:3000"
    command: ["loop"]
    labels:
      - "com.centurylinklabs.watchtower.enable=true"
      - "com.centurylinklabs.watchtower.scope={{PRODUCT}}"
    volumes:
      - {{PRODUCT}}-data:/data:ro
      - "/mnt/truenas/Secure/Application Backups/{{PRODUCT}}:/backups"
      - ./backup.toml:/etc/stash-backup/config.toml:ro
    network_mode: none
```

- The healthcheck is baked into the image (`HEALTHCHECK CMD stash-backup health`): healthy
  while the last success is younger than `max_age`. It reads a local state file, never NFS.
- **`network_mode: none`.** The sidecar needs nothing but its two mounts. Its logs reach Loki
  through promtail's stdout scrape, and its numbers reach Prometheus through stash. The
  MariaDB source is the exception: that sidecar joins the stack's network to reach the
  database.
- **The host must mount `Secure`, and something must restart the sidecar if it started before
  NFS was up.** On dev that is homelab's `nas-guard` timer, which must cover
  `/mnt/truenas/Secure` as well as `Files`. Without it the sentinel turns every early start
  into a failed run: safe, but it never recovers on its own.
- A service run by systemd instead of Komodo runs the same container with its data dir as a
  `:ro` bind mount, which needs uid 3000 to be able to traverse every parent directory.

## Observability

- **One log line per run**, always the run's last: `stash-backup ok product=... snapshot=...
  db_bytes=... duration_s=...` or `stash-backup FAILED product=... reason=... detail="..."`.
  `reason=` is a fixed vocabulary: `config`, `no-target`, `no-database`, `workdir`,
  `sqlite-backup`, `server-started-mid-backup`, `integrity-check`, `foreign-key-check`
  (verify only), `mkdir`, `mirror-copy`, `require-files-query`, `require-files-missing`,
  `require-files-mismatch`, `unsafe-path`, `gzip`, `publish`, `incomplete-snapshot`,
  `mariadb-dump`, `mariadb-check`, `runs-log`, `state`, `no-snapshots`, `no-such-snapshot`,
  `gunzip`, `rename`, `panic`. A `no-target` run can't write `runs.jsonl`, by design; stash
  sees it as staleness. Loki:
  `{container=~".*-backup-1"} |= "stash-backup"`.
- **`runs.jsonl`** holds the same facts, plus the image version that ran, so "did the fix
  reach this sidecar" has an answer.
- **Metrics come from stash**, not the sidecars: `stash_backup_last_success_timestamp_seconds`,
  `stash_backup_last_drill_timestamp_seconds`, `stash_backup_audit_failures` and so on, labelled by
  `product`, on stash's `/api/metrics`. One scrape target, one alert rule for the fleet.

## Restore, verify, drill

- **`verify DIR`** runs `integrity_check`, `foreign_key_check`, every `require_files` query
  and its sha256s, and logs the configured `counts`. It must end with `verify ok`.
- **`restore SNAPSHOT DIR`** (`latest`, or a path under the product directory) unpacks and
  checks the snapshot *before* touching `DIR`, moves the current database **and its `-wal`
  and `-shm`** into `DIR/pre-restore-<stamp>/` (never deletes them; a stale `-wal` beside a
  restored file is replayed into it on open), copies in mirror files `DIR` lacks, gives the
  restored files the owner of the database it moved aside, then runs `verify`. The service must
  be stopped.
- **Drills** are stash's job: nightly, it unpacks each product's newest daily snapshot (the
  databases only) into scratch on its own local disk and runs the same `verify`, checking the
  mirror in place on the NAS. Neither step needs the running service.

## In the repo

- `deploy/compose/backup.toml` and the `backup` service in `prod.compose.yml`. Both are in
  [`templates/deploy/compose/`](../templates/deploy/compose/).
- **`SHORTCUTS.md`** answers "are backups running?" and "how do I restore <product>?" with
  the exact commands for this stack ([shortcuts.md](shortcuts.md)).
- The `deploy/README.md` stack list names the `backup` service and the one-time sentinel
  step.

## Current coverage

As of 2026-10: auth is the only MariaDB source. tavern, dungeon, argus and agentutil still keep
a `secret.key` in their data dir. The game box (dash, warden) mounts neither NAS share, so its
stacks adopt this once it does; warden runs under systemd there.

## Checklist

- [ ] A `backup` service running `stash-backup`, data volume mounted `:ro`, `network_mode: none`
      (unless MariaDB).
- [ ] `backup.toml` lists every database and file tree the service can't rebuild; nothing else.
- [ ] Every row that names a file has a `require_files` query.
- [ ] Destination under `Secure/Application Backups/<product>`, sentinel created on the host.
- [ ] No key that seals stored data lives under `/data`.
- [ ] `SHORTCUTS.md` covers "are backups running" and "how do I restore".
- [ ] The product appears in stash's Backups section with a passing audit and a passing drill.
