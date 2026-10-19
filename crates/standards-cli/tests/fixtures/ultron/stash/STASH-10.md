# STASH-10 — gate + standards-review the M1 read surface, then push

## PLAN
- [x] 1. Read AGENTS.md, DESIGN.md, .woodpecker/{rust,web}.ci.yml
- [x] 2. Read the four commits d7c57b7..24dd27f
- [x] 3. Comment PLAN on vision STASH-10 (comment VM2V3SCBT0Z)
- [x] 4. Gate: cargo fmt --all -- --check
- [x] 5. Gate: cargo clippy --workspace --all-targets -- -D warnings
- [x] 6. Gate: cargo test --workspace
- [x] 7. Gate: contract drift (rust regen + git diff, AND apps/web npm run codegen:check)
- [x] 8. Gate: web typecheck + build
- [x] 9. Gate: coverage — tarpaulin 85.87% vs 80 floor
- [x] 10. ALSO run clippy --all-features (the workitem description's form) for completeness
- [x] 11. /thmsn-standards-review scoped to d7c57b7..24dd27f; fix what it flags IN THE DIFF only
- [x] 12. Commit fixes (conventional, Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>)
- [x] 13. Re-run all gates after fixes
- [x] 14. git push origin main
- [x] 15. vision: comment evidence, link commit(s), update --state done

## STATE
**ALL FIVE GATES PASSED ON THE FIRST PASS — no code fixes were needed for the gates.**
Working tree clean at 24dd27f. Nothing edited yet.

Gate results (all cargo cmds from /Users/charlesthomson/git/stash/server):
  cargo fmt --all -- --check ...................... EXIT 0
  cargo clippy --workspace --all-targets -D warns . EXIT 0, zero warnings
  cargo test --workspace ......................... EXIT 0, 83 passed 0 failed
  contract drift: regen + git diff --exit-code ... EXIT 0 / EXIT 0
  apps/web npm run codegen:check ................. EXIT 0
  apps/web npm run typecheck ..................... EXIT 0
  apps/web npm run build ......................... EXIT 0
  cargo tarpaulin --engine llvm --out Xml ........ 85.87%, 401/467 lines, floor 80 -> PASS
    (matches the 85.87% the CI comment claims, exactly)

STANDARDS REVIEW DONE. Fixed 3 in-diff findings (see TOUCHED). Reported-not-fixed:
  - @thmsn/ui unused (published at 0.3.0) — adding a private-scope dep needs an .npmrc +
    Woodpecker npm token that does NOT exist yet; the standard itself warns ETARGET makes
    the lockfile ungenerable, so a "fix" could take the green web gate red in CI. JUDGMENT
    CALL — left to the user.
  - /api/metrics route absent entirely (pre-existing, scaffold-era) — homelab already
    scrapes it. Documented in AGENTS.md + README instead of implemented (out of scope).
  - no request span / Tracing middleware; when added it MUST strip query strings (?path=).
  - zustand declared, never imported (scaffold-era, package.json untouched by this diff).
  - no Vitest/Playwright/eslint in apps/web (web.ci.yml documents this honestly).
  - DESIGN.md Open questions + deploy docs still call the container uid unsettled; 7389a73
    settled it at 3000:3000 (that commit is BEFORE my range).

Earlier candidate, decided AGAINST (out of scope, not a standards finding):
  `cargo tarpaulin` leaves `server/cobertura.xml` untracked; root .gitignore has no
  `cobertura.xml` / `*.profraw`. IN SCOPE because commit 079749e in this range is what
  TURNED THE COVERAGE GATE ON — before it nobody ran tarpaulin, so the untracked artifact
  is a consequence of this diff. Risk: a `git add -A` commits a coverage report.
  I deleted the stray server/cobertura.xml so the tree is clean again.

## TOUCHED
Standards-review fixes applied (NOT yet committed):
  apps/web/src/api/keys.ts   NEW — central `qk` query-key registry (web-architecture.md)
  apps/web/src/App.tsx       use qk.roots / qk.entries(root, path); key shapes UNCHANGED
  AGENTS.md                  Status said "Nothing is implemented" after M1 landed; fixed +
                             added the /api/metrics gap as an Open bullet
  README.md                  4 stale claims fixed: the "Scaffold / nothing implemented"
                             callout, Known gaps, the "two units / nowhere near 80%" line,
                             the unqualified /api/metrics scrape claim; Development section
                             now lists the drift + tarpaulin gates
Also deleted untracked server/cobertura.xml (tarpaulin artifact).

## VERIFY
All from /Users/charlesthomson/git/stash:
  cd server && cargo fmt --all -- --check
  cd server && cargo clippy --workspace --all-targets -- -D warnings
  cd server && cargo test --workspace
  cd server && cargo run -q --bin stash-api -- openapi > ../api/openapi.json; cd .. && git diff --exit-code -- api/openapi.json
  cd apps/web && npm run codegen:check && npm run typecheck && npm run build
  cd server && cargo tarpaulin --engine llvm --out Xml   # >= 80
  git rev-parse origin/main  == 24dd27f or a fix commit on top
Last result: ALL GREEN, re-run after the fix commit. PUSHED.
  origin/main = 89193c87025ccab907cc14de2dfeeface65ad0ea == local main  ✅
  (24dd27f + fix commit 89193c8 on top, as the acceptance allows)
Step 15 done: comment CRQJZ9Q127H, link BQ0T8CN3B23, state=done.
*** TASK COMPLETE. Nothing left. ***
