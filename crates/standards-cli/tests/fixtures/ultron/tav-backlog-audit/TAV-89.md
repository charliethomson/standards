# TAV-89 — cursor pagination {items,next_cursor} + legacy array shim (implementer journal)

## PLAN
1. Inventory list ops in api/openapi.json + their current SQL order.
2. Server: Page<T> envelope, cursor codec (base64url JSON key tuple, 400 on tamper), helper;
   keyset SQL in db; shim (no page_size => legacy array, undocumented).
3. Server tests per endpoint (walk, insert between pages, tamper 400, shim, messages limit/offset).
4. Regenerate spec + web + apple clients (keep info.version).
5. Web: send page_size; infinite queries / fetch-all helper; tests; Playwright mocks.
6. iOS: send page_size; fetch-all / paging helper; swift test; unsigned sim build.
7. Verify (clippy, coverage>=80, web checks, playwright), commit to main, push, finish comment.

## STATE
- step 1 starting (worktree at 03103ed = origin/main).
  List ops: list_characters, list_character_images, list_chats, list_chat_members,
  list_all_chat_members, list_messages, list_models, list_personas, list_presets,
  list_lorebooks, list_entries, list_links, list_settings.

- steps 2-3 DONE (server): db/src/pagination.rs keys + *_page fns (keyset, LIMIT -1 = legacy);
  api/src/pagination.rs (Page<T>, NextCursor required+nullable, Listing<T> ApiResponse = Json<Page<T>> meta,
  Pager; cursor = ChaCha20 sealed {scope,key} base64url); all 13 handlers converted; messages legacy
  limit/offset read via req.params(). api/src/routes/pagination_tests.rs (all pass), models.rs tests.
  clippy -D warnings clean; fmt -p api -p db clean. Spec regenerated (version kept 0.1.448).
  Web decision: fetch-all helper everywhere (WS sync patches full arrays; message order ASC so newest
  page is last; infinite queries would need reverse cursor) — report as deviation.

- steps 4-6 DONE: web schema regenerated; src/api/pagination.ts (fetchAllPages + PAGE_SIZE) used by
  all 13 list queries; e2e/mock.ts fulfillJson wraps arrays for page_size requests (14 specs delegate).
  iOS (subagent): regenerated, TavernService.fetchAllPages + pages(); listMessages drops limit/offset;
  PaginationTests.swift. Step 7 DONE: 12ad524 pushed to main; finish comment posted. Task complete.

## TOUCHED
- apps/web/src/api/{pagination.ts,pagination.test.ts(new),queries.ts,schema.d.ts}, apps/web/e2e/{mock.ts(new),*.spec.ts}
- apps/apple/Packages/TavernKit/{Sources/TavernAPI/*,Sources/TavernKit/{TavernService,AppModel}.swift,
  Tests/TavernKitTests/{PaginationTests(new),TracingTests,DisconnectResetTests}.swift}
- server/db/{Cargo.toml,src/pagination.rs(new),lib.rs,characters,personas,presets,lorebooks,
  character_images,chat_members,chats,messages,settings}.rs; server/Cargo.lock
- server/api/src/{main.rs,pagination.rs(new),routes/{mod,characters,chats,chat_members,messages,
  personas,presets,lorebooks,settings,models,pagination_tests(new)}.rs}; api/openapi.json

## VERIFY
- cargo test --workspace: all pass (tavern-api 258); clippy --workspace --all-targets -D warnings clean;
  fmt -p tavern-api -p tavern-db --check clean; tarpaulin 87.63% (8747/9982)
- web: codegen stable, typecheck clean, lint clean, vitest 65 files / 1199 passed, playwright 78 passed
- apple: generate-api no drift, swift test 198 passed, xcodebuild Tavern-iOS unsigned sim BUILD SUCCEEDED
