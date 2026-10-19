# STASH-27 — backup crate: MariaDB source (structural checks)

PLAN
- [x] 0. rebase worktree onto main (now 15b21c8)
- [x] 1. read once.rs, config.rs (Mariadb), nas.rs (Staged/Format), gz.rs, runs.rs (MariadbRun), sqlite.rs as pattern; STASH-20..24 comments
- [x] 2. vision start comment with PLAN
- [x] 3. config: `mariadb_dump` binary path field (default "mariadb-dump")
- [x] 4. src/mariadb.rs: dump (child env MYSQL_PWD), checks (exit, trailer, gzip round-trip), stderr scrub, stage via nas::Staged Format::Mariadb
- [x] 5. once.rs: replace LD-28 placeholder with the mariadb step after require_files; publish in the all-or-none set; MariadbRun in RunRecord
- [x] 6. tests: success / nonzero exit / missing trailer / password channel + not leaked / real-dump #[ignore]
- [x] 7. gates (fmt, build, clippy -D warnings, test); README/AGENTS updates if behaviour described there
- [x] 8. commit `backup: the MariaDB source (STASH-27)`; finishing vision comment

STATE
- DONE: committed 2cae629, finishing comment posted on STASH-27.

TOUCHED
- server/backup/src/config.rs, mariadb.rs (new), mariadb/tests.rs (new), once.rs, once/tests.rs, lib.rs; server/README.md
- commit 2cae629 on worktree-agent-adb1fe0bf5339af97 (parent 15b21c8)

VERIFY
- cd server && cargo fmt --all -- --check && cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
- last result: all green; stash-backup 118 passed 1 ignored; llvm-cov backup 94.5% lines (mariadb.rs 93.4%, once.rs 93.9%)
