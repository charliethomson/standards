# STASH-19 — rust-advisories: quick-xml via poem, rsa in the lockfile
Worktree: /Users/charlesthomson/git/stash/.claude/worktrees/agent-ad49850167f84fd3d

PLAN
- [x] 1. Baseline `cd server && cargo audit`; vision in_progress + start comment
- [x] 2. quick-xml: newer poem/poem-openapi 3.x/5.x? feature to drop? else allowlist with proof
- [x] 3. rsa: why in lock (sqlx-macros-core -> sqlx-mysql?); make vanish or allowlist with reason
- [x] 4. rustls-pemfile: leave unless trivially removable
- [x] 5. fmt/clippy/test, contract drift check, commit (no push), finish comment

STATE
- quick-xml: no newer poem/poem-openapi of any major; poem-openapi depends on quick-xml unconditionally + enables poem/xml -> allowlisted w/ proof + test no_operation_speaks_xml
- rsa: lock-only via sqlx weak features (cargo#10801) -> allowlisted
- rustls-pemfile: via auth-sdk (other repo) -> left
- DONE. Commit e35bd13 (not pushed). Finish comment posted; issue left in_progress for orchestrator.

TOUCHED
- server/.cargo/audit.toml (new)
- server/api/src/app.rs (test no_operation_speaks_xml)
- vision: in_progress + start comment

VERIFY
- cd server && cargo audit; cargo fmt --all -- --check; cargo clippy --workspace --all-targets --all-features -- -D warnings; cargo test --workspace
- cargo run --bin stash-api -- openapi > ../api/openapi.json(?) ; git diff --exit-code api/openapi.json
- LAST RESULT: cargo audit exit 0 (1 allowed warning); fmt clean; clippy clean; 410 tests pass; openapi no drift
