# STD-22 journal: port bin/janitor to `standards janitor`
Worktree: /Users/charlesthomson/git/standards/.claude/worktrees/agent-acbdbce957057a241 (branch std-22 off origin/main 74be252)

PLAN
- [x] 1. setup: fetch, checkout -B std-22 origin/main; read bin/janitor, cli skeleton, SPECS
- [ ] 2. vision start comment on STD-22 with this plan
- [ ] 3. implement janitor.rs + janitor/*.rs (scan/clean, all flags, --json, human output via io.out)
- [ ] 4. tests: fixture tree builder (temp dir, canonicalized), verdict asserts, clean --yes asserts, prompt y/n, non-tty error, golden bash-vs-rust scan diff test
- [ ] 5. fix tests/cli.rs (janitor no longer "not implemented")
- [ ] 6. bin/janitor deprecated header comment (no behaviour change)
- [ ] 7. crates/README.md + CHANGELOG.md
- [ ] 8. cargo test --workspace, clippy, llvm-cov standards-cli >=80%
- [ ] 9. commit, fetch, rebase origin/main, retest, push HEAD:main; delete target dir
- [ ] 10. vision finish comment (SHA, evidence, deviations)

STATE
- on step 2

TOUCHED
- (none)

VERIFY
- cargo test --workspace ; cargo clippy --workspace --all-targets -- -D warnings ; cargo llvm-cov -p standards-cli --fail-under-lines 80
- last result: not run
