# Spec: the `stash-backup` crate, binary and image (canonical workstream)

Source: a spec agent's read of stash @ main (80b7cb2), vision-backup.sh and libconfig, reconciled by
Jarvis 2026-10-13. The contract is `~/git/standards/docs/backups.md`. Where this file and the
standard disagree, the standard wins and the disagreement is a bug to report.

## Placement

- One new package: `server/backup`, package name `stash-backup`, lib + bin (`src/main.rs`).
  Add to `members` (`server/Cargo.toml:3`) and `[workspace.dependencies]`; add `backup/src/main.rs`
  to tarpaulin `exclude-files` (`server/tarpaulin.toml:7`). Layering in `server/README.md` becomes
  `core → db/storage/backup → thumbs → engine → api`.
- **Depends on no `stash-*` crate.** stash-engine (drills) and stash-api (reads) depend on it.
- Synchronous (std::fs + rusqlite). stash calls it via `spawn_blocking`. The binary uses a
  current-thread tokio runtime only for libsignal (async API, `libsignal/src/lib.rs:132,215`).
- Re-implement, don't import, from `server/storage`:
  - sentinel: just "`<dest>/.<product>-backup-target` is a file" (stash's is `.stash-root` + RootDegraded, `storage/src/lib.rs:44,92-111`)
  - atomic write: `document::write_temp` + `sync_dir` + rename shape (`storage/src/document.rs:141,153-186`), sync, `.partial` naming
  - path safety: `path::resolve_lexical` (`storage/src/path.rs:24-50`) → `backup::path::safe_join` with its own error

## SQLite

- `rusqlite = { version = "=0.32.1", features = ["bundled", "backup"] }`. sqlx 0.8 bundles
  `libsqlite3-sys 0.30.1` (`Cargo.lock:1645-1653`, `links = "sqlite3"`), only one may exist in the
  graph. Acceptance: `cargo tree -i libsqlite3-sys` shows one version. Comment in Cargo.toml:
  bumping sqlx bumps rusqlite in the same commit.
- Open `file:<path>?mode=ro` with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI`, `busy_timeout(30s)`.
- **Journal mode from the header** (bytes 18-19 == 2 ⇒ WAL). Only for WAL with neither `-wal` nor
  `-shm` present: add `&immutable=1`, fingerprint `(len, mtime_ns, ino)` before, re-check after,
  discard with `server-started-mid-backup` if changed or a `-wal`/`-shm` appeared. Rollback-journal
  DBs (registry, argus, translate, news, agentutil) never use immutable.
- `Backup::new` + one `step(-1)`: a single consistent read transaction.
- Snapshot into the container's local `/tmp` work dir, `integrity_check` (fail run),
  `foreign_key_check` (record count, don't fail; LD-3), `counts` (null + warn on failure; LD-9),
  gzip, then publish.

## MariaDB (LD-2)

- Shell out to `mariadb-dump --single-transaction` (no Rust equivalent; sqlx-mysql is kept unbuilt,
  see `.cargo/audit.toml`). Password via child env `MYSQL_PWD` or a 0600 defaults-extra-file in the
  work dir. Never argv, logs, or the effective config.
- Output `db/hourly/<database>-<stamp>.sql.gz` (+ daily). Check: exit 0, trailer contains
  `-- Dump completed`, gzip round-trips. No load into a server in v1.
- Client must be MariaDB 11.x (auth runs `mariadb:11`, `auth/deploy/compose/prod.compose.yml:17`).
  Base the runtime on `debian:trixie-slim` (MariaDB 11.8 client) or MariaDB's apt repo pinned to 11.
- `mariadb-dump` path is config so tests can use a fake script fixture; a real-dump test is
  `#[ignore]` unless the binary is on PATH.

## Config

- `Loader::path("/etc/stash-backup/config.toml").env_prefix("STASH_BACKUP_").shared_env([])`.
  libconfig (rev 5fee793) has no `.split()`: env overrides **top-level scalars only**.
- Top-level: `product` (required, `^[a-z][a-z0-9-]*$`), `data_dir` (`/data`), `dest` (`/backups`),
  `state_dir` (`/tmp/stash-backup`), `mariadb_password` (secret, `#[serde(skip_serializing)]`).
- Hand-written `Default` (configuration.md): minute 7, max_age 2h10m, hourly 48, daily 30.
- libconfig silently ignores a missing file ⇒ empty `product` after load ⇒ `reason=config`.
- Validation (`reason=config`): every `require_files.mirror` names a `[[files]].name`; db names
  (`[[sqlite]].name`, default file stem; mariadb database) unique; `[[files]]` paths don't nest and
  are under `data_dir`; `[[files]].name` one safe component; at most one `[[mariadb]]`; minute 0..=59;
  retention ≥ 1.

## On-NAS contract (stash consumes this)

`.stash-backup/config.toml`, rewritten each run via `.partial` + rename:

```toml
schema = 1
written_at = "2026-10-13T14:07:05Z"
version = "0.1.214"          # image version (RELEASE_VERSION via build.rs)
# then the fully resolved Config, defaults filled, password omitted
```

`.stash-backup/runs.jsonl`: one line per run (sentinel present, ok or failed), single `write_all`
on an O_APPEND handle; readers skip a torn last line. No rotation in v1 (~5 MB/yr/product).

```json
{"schema":1,"product":"vision","version":"0.1.214","trigger":"loop","started_at":"…Z","finished_at":"…Z","duration_s":4.1,
 "outcome":"ok","reason":null,"detail":null,"snapshot":"20261013T140700Z",
 "sqlite":[{"name":"vision","source":"wal|quiescent|rollback","bytes":1234,"gz_bytes":456,"hourly":"db/hourly/vision-20261013T140700Z.db.gz","daily":null,
            "integrity":"ok","foreign_key_violations":0,"counts":{"workitem":12,"workitem_attachment":3}}],
 "files":[{"name":"attachments","total":3,"copied":1,"bytes_copied":5555}],
 "require_files":[{"db":"vision","mirror":"attachments","rows":3,"missing":0}],
 "mariadb":[],"pruned":["db/hourly/vision-20261011T130700Z.db.gz"]}
```

Log lines (stdout, raw, exact prefix; LD-10). The per-run line is the run's last:
`<RFC3339Z> stash-backup ok product=… snapshot=<stamp> db_bytes=<sum> files_copied=… duration_s=…`
`<RFC3339Z> stash-backup FAILED product=… reason=<r> [db=…|path=…|count=…] detail="…"`
Aux: `stash-backup pruned path=…`, `verify ok …`, `restored …`, `starting version=…`.
`reason=` vocabulary: as in backups.md "Observability" (derived from vision-backup.sh lines 96-268).

## Library API (stash calls this; pacing per LD-11)

```rust
pub struct Placement { pub dbs: BTreeMap<String, PathBuf>, pub mirrors: BTreeMap<String, PathBuf> }
impl Placement {
    pub fn data_dir(cfg: &Config, dir: &Path) -> Result<Self, BackupError>;   // verify DIR / restore
    pub fn drill(cfg: &Config, scratch: &Path, product_dir: &Path) -> Self;  // dbs in scratch, mirrors on NAS (read only)
}
pub struct Pace { pub bytes_per_sec: Option<u64> }                            // throttles sha256 reads
pub fn verify(cfg: &Config, at: &Placement, pace: &Pace) -> VerifyReport;     // never Err; .outcome()/.reason()
pub fn drill(product_dir: &Path, scratch: &Path, which: SnapshotSel, pace: &Pace) -> Result<DrillReport, BackupError>;
pub fn read_runs(product_dir: &Path, tail: usize) -> Result<Vec<RunRecord>, BackupError>;
pub fn read_effective(product_dir: &Path) -> Result<EffectiveConfig, BackupError>;
pub fn list_snapshots(product_dir: &Path) -> Result<Vec<SnapshotSet>, BackupError>;
pub fn parse_snapshot_name(name: &str) -> Option<(String /*db*/, Tier, DateTime<Utc>)>;
```

`drill` reads the effective config from `product_dir`, resolves `SnapshotSel{Latest|Stamp|Daily}` to
a complete set, gunzips **only the databases** into scratch (local disk), runs verify, never writes
under `product_dir`. Verify fails on FK violations (LD-3).

## Run order (`once`)

sentinel → work dir → each sqlite snapshot (+ checks) → mariadb dump → file mirrors (additive,
recursive, skip symlinks/non-regular; new copies sha-checked against require_files rows before
rename, LD-7) → require_files presence → publish all dbs together (any failure ⇒ none published) →
daily-first → prune by name (failure logged, non-fatal) → runs.jsonl + effective config + local
state + `last-success` → log line.

`restore`: unpack+check before touching DIR; move db, -wal, -shm to `DIR/pre-restore-<stamp>/`;
add missing mirror files; chown restored files to the moved-aside db's owner (LD-8); verify.
`health`: local state file vs `max_age`. `loop`: `next_run(now, minute)` pure fn; SIGTERM during
sleep exits cleanly; a panicking run logs `reason=panic` and the loop continues.

## Image

- `Dockerfile.backup` at repo root (context `.`, root `.dockerignore` applies). Build stage as
  `Dockerfile:12-42` (rust:1-bookworm, ssh mount, cache mounts) with `--bin stash-backup`.
  `backup/build.rs` copies `server/api/build.rs`'s RELEASE_VERSION handling.
- Runtime `debian:trixie-slim` + ca-certificates + mariadb-client (11.x); uid/gid 3000;
  `ENTRYPOINT ["/usr/local/bin/stash-backup"]`, `CMD ["loop"]`;
  `HEALTHCHECK --interval=5m --timeout=10s --start-period=10m --retries=1 CMD ["/usr/local/bin/stash-backup","health"]`.
- `.woodpecker/backup-image.build.yml`, copy of `image.build.yml`; paths `server/**`, `scripts/ci/**`,
  `Dockerfile.backup`, `.dockerignore`, itself; `sh scripts/ci/docker-build.sh stash-backup Dockerfile.backup . --ssh default`.

## Tests (behaviour, tempdirs, injected now, no mocks)

Each vision-backup.sh edge case becomes a test: WAL data only in -wal (writer held open,
`wal_autocheckpoint=0`); quiescent leaves dir byte-identical; started-mid-backup via the guard API;
only one of -wal/-shm ⇒ normal path; rollback-journal DB never immutable; corrupt page ⇒
integrity-check, nothing published; partials never under final names, stale `.partial` ignored by
prune/latest; prune by name with reversed mtimes, other products' files untouched, prune only after
publish; daily-first across UTC days; missing sentinel ⇒ dest unchanged; additive mirror never
overwrites; require_files missing/mismatch/unsafe-path; multi-db atomicity; restore (corrupt
snapshot leaves DIR untouched, -wal/-shm moved aside, ownership); `latest` = newest complete set;
password never in effective config; health fresh/stale/missing; `next_run` table incl. :07:00 and :59.
CI runs as root: assert "no file created", not chmod failures.
