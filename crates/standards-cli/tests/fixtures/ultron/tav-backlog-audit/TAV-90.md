# TAV-90 — flip create handlers to 201

## PLAN
1. mod.rs: replace Created::ok with Created::new (201, only constructor); handlers use it.  DONE
2. Tests: create_still_answers_200 -> create_answers_201; per-route 201 asserts in existing route tests (all 18 ops).  DONE
3. Web mocks/unit tests: no status===200 dependence on creates (mocks answer 200, still documented).  DONE
4. clippy + cargo test --workspace; spec emit identical; web codegen:check clean.  DONE
5. Commit to main, push, comment.

## STATE
- DONE: ba18948 pushed to main (rebased on TAV-102 b067de2); finish comment + count correction posted; re-verified post-rebase (271 api tests, clippy, codegen:check clean)
- NOTE: Created variant doc comments are the spec response descriptions -> kept verbatim so openapi.json is unchanged (TAV-91 rewords)
- NOTE: clippy self_named_constructors rejected `Created::created`, so constructor is `Created::new`

## TOUCHED
server/api/src/routes/{mod,assets,characters,chat_members,chats,import,import_resources,lorebooks,personas,presets}.rs, server/api/src/openapi.rs (tests)

## VERIFY
- clippy -D warnings clean; cargo test --workspace all ok (tavern-api 258 passed)
- fresh `tavern-api openapi` == api/openapi.json except info.version (build stamp)
- apps/web npm run codegen:check: no diff
