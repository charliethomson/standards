# STASH-28 — stash-backup image and Woodpecker pipeline

PLAN
- [x] 0 rebase worktree branch onto main (36ef80f)
- [x] 1 read context: sidecar.md "Image", PROGRAM.md LD-2/5/10/29, STASH-28 comments, backup config/mariadb.rs/build.rs
- [x] 2 vision comment: start + plan
- [x] 3 Dockerfile.backup
- [x] 4 .woodpecker/backup-image.build.yml + AGENTS.md secrets table (+ README/deploy docs if they list pipelines)
- [x] 5 docker up; build image locally (version.sh -> RELEASE_VERSION, --ssh default)
- [x] 6 inspect (User, Healthcheck), mariadb-dump --version, image size
- [x] 7 smoke run: tmp dest + sentinel + sqlite + backup.toml -> ok line; runs.jsonl version
- [x] 8 throwaway mariadb:11, dump through image -> TLS finding; fix in mariadb.rs + config + test if needed
- [x] 9 cargo gates (if Rust touched); cleanup containers/images/builder cache
- [x] 10 commit; vision finishing comment

STATE
- DONE. Commit 1cc5812; vision finishing comment posted. Throwaway containers/network/images removed.

TOUCHED
- Dockerfile.backup (new), .woodpecker/backup-image.build.yml (new), AGENTS.md + deploy/README.md secrets tables, README.md layout row, deploy/README.md deploy-model para. Commit 1cc5812 (also server/backup/src/mariadb.rs doc comment).

VERIFY
- scratchpad s28-build.sh 0.0.51 ; s28-smoke.sh setup/once/show/loop ; docker inspect User=3000:3000 + healthcheck ; cargo fmt --check, clippy -p stash-backup -D warnings, test -p stash-backup (152+7 pass) — all green 20:20Z
