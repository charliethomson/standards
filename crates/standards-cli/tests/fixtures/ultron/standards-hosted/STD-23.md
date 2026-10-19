# STD-23 Port bin/ultron to `standards ultron`

PLAN
- [ ] 1. read bin/ultron, cli skeleton (cli.rs, ultron.rs, env.rs, io, outcome), pick fixtures
- [ ] 2. comment start on vision STD-23 with PLAN
- [ ] 3. copy fixtures (>=3 programs) to crates/standards-cli/tests/fixtures/ultron/<slug>/, pin mtimes, capture bash goldens (ls, status) with pinned clock (faketime or normalised time column)
- [ ] 4. implement ultron/{parse,render,cmd} : ls, status, watch, log, gc, help, --json; Env gets ultron_state_dir + now
- [ ] 5. golden tests + unit tests (fingerprint, watch non-TTY, parse edge cases), coverage >=80%, clippy
- [ ] 6. bin/ultron deprecated header, crates/README.md, CHANGELOG.md
- [ ] 7. commit, rebase origin/main, test+clippy, push
- [ ] 8. live side-by-side diff vs real state dir (read-only ls/status), vision finish comment

STATE
- step 1 in progress. branch std-23 from origin/main 74be252 in worktree agent-a752d2ba689342c33.

TOUCHED
- (none yet)

VERIFY
- cargo test --workspace ; cargo clippy --workspace --all-targets -- -D warnings ; cargo llvm-cov -p standards-cli
- diff <(ultron ls) <(target/debug/standards ultron ls) ; same for status standards-hosted
- last result: not run
