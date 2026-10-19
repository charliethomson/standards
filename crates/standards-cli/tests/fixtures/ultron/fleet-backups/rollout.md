# Spec: per-repo rollout facts (survey 2026-10-13)

All stacks are Komodo `server = "dev"` except dash (`game`) and warden (systemd user unit on the
game box, not Komodo). Hosts: `homelab/komodo/sync/servers.toml` (dev 192.168.0.193, game 192.168.0.87).

## Cross-cutting

- `nas-guard` only covers `/mnt/truenas/Files` (`homelab/media/host/nas-guard.sh:47`, filter `:103`).
  `/mnt/truenas/Secure` is a separate NFS mount on dev (`stash/DESIGN.md:705`). → HOMELAB task.
- The game box mounts neither share and has no nas-guard. → HOMELAB task, blocks dash + warden.
- WAL off (rollback journal) in registry, argus, translate (also news, agentutil, out of program).
- `secret.key` under /data: tavern, dungeon, argus (and agentutil). dash has none; its compose
  comment `dash/deploy/compose/prod.compose.yml:96` is wrong.
- Every rollout: `deploy/compose/backup.toml` from `standards/templates/deploy/compose/backup.toml`,
  `backup` service from the template compose, SHORTCUTS "are backups running?" + "how do I restore?"
  (vision's are the model), deploy/README stack list. Host step (human): mkdir + sentinel under
  `/mnt/truenas/Secure/Application Backups/<product>`.

## Per repo

**vision (VIS)** `deploy/compose/prod.compose.yml`, `vision-data`→/data (`:124`). `vision.db`
(`server/db/src/lib.rs:60`), WAL (`:73`). `attachments/<uuid>` source data, sha256 on row
(`lib.rs:156`, `repo/attachment.rs`), never deleted/rewritten. Key in env. liblog JSON log under
/data (`LIBPATH_BASE_DIR=/data`, `:30`): don't list. Current sidecar `:173-212` writes to
`Files/11 - Application Backups/vision` (`:202`). require_files query is in backups.md.

**tavern (TAV)** `tavern-data`→/data (`:102`). `tavern.db` (`server/db/src/lib.rs:137`), WAL (`:148`).
`assets/<hh>/<sha256>` source data (rows: `path`, `content_hash`), written in place without rename
(`db/src/assets.rs:67`), orphans unlinked by `gc_orphaned_assets` (`assets.rs:184-246`); a crash can
leave a partial file the next store rewrites. The sha-check-before-mirror rule (LD-7) keeps a
partial out of the mirror; GC deletions just leave files in the mirror. `thumbs/` cache: exclude.
`libpath/` logs: exclude. `secret.key` (`server/api/src/crypto.rs:15,29-30,98`) seals the OpenRouter
key. Image has no USER (root): check the sidecar (uid 3000) can read /data files.

**auth (AUTH)** MariaDB `maria` service, `container_name: auth-maria`, `mariadb:11`,
`auth-db:/var/lib/mysql` (`deploy/compose/prod.compose.yml:16-24`). Database `auth` (`:22`). Everything
connects as root with `[[AUTH_DB_PASSWORD]]` (`deploy/komodo/sync/stacks.toml:34`). Backup user:
the `seed` one-shot runs as root on every deploy (`stacks.toml:32`, `db/seed/src/main.rs`); have it
idempotently `CREATE USER IF NOT EXISTS 'backup'` + `GRANT SELECT, LOCK TABLES, SHOW VIEW, TRIGGER,
EVENT ON auth.*` from a new env var, password a new Komodo secret Variable. Sidecar joins network
`auth`, not `network_mode: none`. `acme_state`, `auth_tcp_certs` rebuildable. No SHORTCUTS.md yet.

**registry (REG)** `registry-data`→/data (`:48`), `verdaccio-storage` (`:85`). `registry.db`
(`server/db/src/lib.rs:108`), rollback journal. `blobs/<product>/<app>/<version>/<file>`
(`server/store/src/lib.rs:80,102-118`) source data, atomic writes (`:217`), rows hold file_name +
sha256 (path is rebuilt from product id/app slug/version: the require_files query must join).
Whole version/app/product dirs deleted (`:247-276`). If a deleted version can be re-uploaded under
the same name with new bytes, the additive mirror keeps the old file and require_files would fail
on sha: find out, and raise it if so. `assets/<hash>` icons/screenshots, source data. Verdaccio
excluded in v1 (LD-21). No SHORTCUTS.md, no deploy/README.md.

**dungeon (DUN)** `dungeon-data`→/data (`:80`). `dungeon.db` (`server/db/src/lib.rs:96`), WAL (`:106`).
No file trees. `secret.key` (`server/db/src/crypto.rs:19,48,119`) seals provider keys. Runs as uid
10001 (`Dockerfile:59,70`): check uid 3000 can read.

**argus (ARGUS)** `argus-data`→/data (`:51`). `argus.db` (`server/db/src/lib.rs:48`), rollback
journal. `secret.key` (`server/db/src/crypto.rs:26,49,115`; `lib.rs:53-55`) seals booru creds.
No SHORTCUTS.md, no deploy/README.md.

**translate (TRANS)** `translate-data`→/data (`:68`). `translate.db` (`server/api/src/config.rs:188`),
rollback journal (`server/db/src/lib.rs:54-62`). No file trees, no key on disk.

**dash (DASH)** game box. `dash-data`→/data (`:97`). `dash.db` (`server/crates/db/src/pool.rs:30`),
WAL (`:44`). No key on disk (env, `bin/api/src/config.rs:6,103-115`); fix the compose comment at
`:96`. `/srv/dash` game data and its backups are the product's (out of scope). No SHORTCUTS.md.

**warden (WARD)** game box, systemd user unit for `c`. `data_dir=/home/c/.local/share/warden`
(`deploy/config.example.toml:9`, `server/api/src/config.rs:130-131`), `warden.db`
(`server/db/src/lib.rs:29`), WAL (`:34`). World backups default to `<data_dir>/backups/<id>`
(`config.rs:99`): list only `warden.db`, never the backups dir. Secrets in `~/.config/warden/env`.
Sidecar runs as docker on the game box with a ro bind mount; uid 3000 must traverse `/home/c`
(check its mode). Not Komodo: document the container in deploy/README + a compose file under deploy/.

**stash (STASH)** `stash-data`→/data (`:88`), api uid 3000. `stash.db` (`server/api/src/main.rs:338`),
WAL (`server/db/src/lib.rs:80`). Index tables rebuildable; `session` isn't (losing it signs everyone
out). `thumbs/` and `drill/` are caches: exclude. No key on disk.
