# TAV-105 journal — TRACE DISPLAY half (web TraceDialog + iOS TraceSheet)

Worktree ~/git/tavern-wt-105t, branch tav105t off integ/tav-2026-09-30 (b7abca9). NO PUSH.

## PLAN
1. Web: lib/traceRetrieval.ts (pure formatting: tier titles, skip reasons, chunk outcomes, lore tier badge) + tests; TraceDialog "Retrieval" section per tier (status/model/threshold/candidates/best/duration/top-k/budget/position; chunks w/ score, tokens, outcome; recall message chunks get "Jump" -> close dialog + select message in pane via useSelection); LoreRow tier badge; ChatPane wiring; TraceDialog.test.tsx (used/skipped tiers, badges, old trace without fields).
2. iOS: TavernKit TraceRetrieval.swift (same formatting logic) + Tests/TraceRetrievalTests.swift; TraceSheet Retrieval sections + lore badge; jump via MessageRow -> chat ScrollViewReader.
3. Verify: web typecheck/lint/vitest; swift test; unsigned sim build; screenshots from fixtures -> tav105t-*.png.
4. Commit on tav105t; comment TAV-105 finish.

## STATE
- DONE: web 602c2b5, iOS f2bcf9a on branch tav105t (not pushed).
- started 2026-09-30; viewers: web apps/web/src/components/chat/TraceDialog.tsx (opened from ChatPane inspectingId), iOS apps/apple/iOS/UI/ChatsView.swift TraceSheet (MessageRow context menu "Inspect prompt").

## TOUCHED
- iOS (f2bcf9a): TavernKit/TraceRetrieval.swift (TraceRetrievalFormat), Tests/TraceRetrievalTests.swift, iOS/UI/ChatsView.swift (TraceSheet retrieval sections + lore badge; MessageRow locateMessage/onJumpToMessage; chat jumpTarget -> proxy.scrollTo + select)
- web (602c2b5): src/lib/traceRetrieval.ts(+test), src/test/traceFixtures.ts, components/chat/TraceDialog.tsx(+TraceDialog.test.tsx), ChatPane.tsx (traceNav locate/jump via useSelection), api/types.ts (RetrievalTrace/RetrievedChunk aliases), e2e/trace.spec.ts (+1 retrieval test)

## VERIFY
- iOS: swift test 221 passed (9 new TraceRetrievalTests); one earlier run had 1 unrelated flaky failure while the sim/maestro were busy, 3 reruns clean. Unsigned sim build (-scmProvider system CODE_SIGNING_ALLOWED=NO, fresh sim tav105t-trace DDB97E14) BUILD SUCCEEDED. Visual check against a no-auth python mock on 127.0.0.1:8788 (scratchpad trace_mock.py) -> tav105t-ios-lore-recall.png, tav105t-ios-recall-chunks.png, tav105t-ios-jumped.png (jump dismisses sheet + selects message 1)
- web: typecheck ok, lint ok, vitest 69 files / 1233 passed, playwright 80 passed; screenshot tav105t-web-trace.png (from e2e fixture)
