# TAV-91 journal
## PLAN
1. Created<T>: drop 200 variant, reword docs, update spec test; Swift CreateOutput .created only; tests.
2. Lists: remove bare-array shim + limit/offset on list_messages; simplify Listing<T>; tests; e2e/mock.ts always envelope.
3. Regenerate spec + web + apple clients.
4. Verify: cargo test, clippy, coverage; web checks + playwright; swift test + sim build. Commit/push main.
## STATE
started
## TOUCHED
## VERIFY
- [step1+2 server] Created<T> 201-only; Listing<T> = Json<Page<T>> alias; Pager no Mode/legacy; fetch_limit -> i64; list_messages limit/offset removed; tests updated. api/openapi.json regenerated (version kept 0.1.448). tavern-api 271 pass.
- USER CHANGE: do NOT push; commit on worktree branch only.
## STATE
committed db943cf on branch worktree-agent-af86da795f8140e3b (NOT pushed per user change of plan)
## TOUCHED
server/api/src/{pagination.rs,routes/*}, api/openapi.json, apps/web/{e2e/mock.ts,src/api/{client.created.test.ts,pagination.ts,schema.d.ts}}, apps/apple/Packages/TavernKit/{Sources/TavernKit/CreateOutput.swift,Tests/.../CreatedStatusTests.swift,Sources/TavernAPI/*}
## VERIFY
clippy -D warnings clean; cargo test --workspace all ok (tavern-api 271); tarpaulin 88.15%
web codegen:check clean, typecheck, lint clean, vitest 65 files/1197 pass, playwright 78 pass
swift test 198/0 failures; unsigned sim build BUILD SUCCEEDED
