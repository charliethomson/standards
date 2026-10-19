# TAV-88 — document 200+201 on creates; clients accept both

## PLAN
1. Inventory create POSTs in api/openapi.json
2. Server: generic `Created<T>` ApiResponse (200+201), handlers return 200; tests
3. Regenerate spec (keep info.version), web client, apple client
4. Swift: helper accepting .ok/.created; update call sites; test with stubbed 201
5. Web: check status===200; test mocked 201
6. Verify all, commit to main, push

## STATE
- server done (Created<T> in routes/mod.rs, 18 ops), spec regen (version kept 0.1.448)
- web codegen + client.created.test.ts done
- workspace clippy + tests green
- Swift CreateOutput.swift helper + CreatedStatusTests done
- DONE: committed 03103ed, pushed to origin main; finish comment posted; state left unchanged
- start comment posted

## TOUCHED
- server/api/src/routes/{mod,characters,chats,chat_members,personas,presets,lorebooks,assets,import,import_resources}.rs
- api/openapi.json, apps/web/src/api/schema.d.ts, apps/web/src/api/client.created.test.ts
- apps/apple/Packages/TavernKit/Sources/TavernAPI (generated)

## VERIFY
- cargo clippy --workspace --all-targets -D warnings: clean
- cargo test --workspace: all ok
- web: codegen:check clean, typecheck, lint, 1190 tests pass
- swift test 193 pass; apple codegen drift clean; unsigned sim build SUCCEEDED
