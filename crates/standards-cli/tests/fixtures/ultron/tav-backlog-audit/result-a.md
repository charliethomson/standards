# TAV backlog audit — result A (TAV-38, TAV-39, TAV-40, TAV-42)

Audited against `main` @ e62c16d on 2026-09-30. Read-only. PARITY.md columns are Server | Web | iOS.

---

## TAV-38 — LLM speaker selection for group chats — **PARTIAL** (server done; one acceptance item + iOS missing)

Implemented in a69ddcb (2026-07-30, "let a group chat delegate turn-taking to an LLM director"); later extended by 21cec8d (natural/pooled modes, 2026-09-22) and client regen in 89e2ad0.

| Requirement | Status | Evidence |
|---|---|---|
| Per-chat mode setting (round-robin / LLM director / manual) | MET | `SpeakerMode {RoundRobin, Llm, Manual, Natural, Pooled}` server/core/src/model.rs:198; column server/db/migrations/0019_speaker_mode.sql |
| Default unchanged | MET | `#[default] RoundRobin` model.rs:201; `DEFAULT 'round_robin'` in 0019 |
| New GenerationKind so spend is separately visible | MET | `GenerationKind::Speaker` → `"speaker"` server/db/src/generations.rs:34,46; recorded at server/api/src/ws/speaker.rs:201. Web spend page has no label for it (falls back to raw `speaker` string, apps/web/src/routes/SpendPage.tsx:10-15,84) — visible but unlabelled |
| Own model setting, cheap default | MET (server only) | `speaker_model` setting + cheap default, never falls through to `default_model` — speaker.rs:44-46,210-214. No client UI sets `speaker_model` (grep of apps/ finds nothing) |
| Input: short window of recent turns, member list with card blurbs, last speaker | MET | `director_brief` server/engine/src/prompt/group.rs (tests `director_brief_transcript_is_windowed_and_attributed`, `..._names_the_last_speaker`, engine/tests/group.rs:248,268); cards loaded speaker.rs:232 |
| Output: one member id + one-line reason, constrained to member set | MET | `parse_director_choice` in group.rs; reason sent as `speaker.selected` frame generate.rs:504-512 |
| Hallucinated/invalid selection → silent round-robin fallback | MET | speaker.rs:67-150 (garbage/no-member/upstream error/timeout all return None → ring); logged at debug only |
| Starvation floor | MET | `starved_speaker` in group.rs; runs before the model call (speaker.rs:81); tests engine/tests/group.rs:393-421 |
| Latency: measured | MET | 2.5s cap speaker.rs:53; metrics `tavern_speaker_selection_total{outcome}` / `_duration_seconds` server/api/src/metrics.rs:30-31,238 |
| Latency: speculative selection during typing | NOT DONE (was "consider", not acceptance) | no speculative path found |
| Muted members excluded | MET | muted never enter brief; test `parse_director_choice_rejects_a_muted_member` group.rs tests:341, `director_brief_lists_only_unmuted_members`:207 |
| Unit tests for fallback paths; group.rs stays pure | MET for pure paths | group.rs imports only std + tavern_core (group.rs:25-29); parse-rejection tests group.rs tests:317-377. The I/O fallbacks (timeout, upstream error, no key) in server/api/src/ws/speaker.rs have **no tests** (no `#[test]` in the file, none in server/api/tests) |
| PARITY.md updated | **MISSING** | a69ddcb did not touch PARITY.md; no row mentions the LLM director / speaker modes (only the pre-existing generic "Group chats (turn-taking, muting)" row, PARITY.md:51) |

Clients: **web only.** Web exposes the mode picker (`llm` → "Director", apps/web/src/lib/speakerModes.ts:26; GroupSettings.tsx:72). Web does not consume the `speaker.selected` frame (reason not shown — not a stated requirement). **iOS has no speaker-mode UI** — `speakerMode` appears only in TavernKit test fixtures and generated telemetry (apps/apple/.../Fixtures.swift, ClientEvents.generated.swift); 21cec8d explicitly says "iOS gets no new UI yet".

---

## TAV-39 — Character & lorebook linter — **PARTIAL** (static engine half only; no API, no client, no model-assisted checks, no PARITY)

Implemented in 9a5d912 (2026-07-30, "static lint for cards and lorebooks") — commit message itself states "No schema change, no API surface yet."

| Requirement | Status | Evidence |
|---|---|---|
| Static: keys that can never fire | MET | server/engine/src/lint/lore.rs; tests engine/tests/lint.rs:364 (no keys), :494 (zero scan depth), :511 (regex/macro key) , :412 (0% trigger) |
| Static: key collisions between entries | MET | tests lint.rs:637, :665 (across books), :683 (shadowed) |
| Static: over-broad keys (everyday words, character's name) | MET | tests lint.rs:542 (short key), :554 (everyday word), :565/:583 (key in card / card name fires every turn) |
| Static: token bloat per entry / whole book vs budget using budget estimator, show what drops first | MET | `BudgetProjection`/`ProjectedEntry` lint/lore.rs; tests lint.rs:720-865 (replays scanner cut rule) |
| Static: disabled / orphaned entries and books | MET (books) | disabled entry :438, all-disabled book :465, unlinked book error :333. "Entry linked to no book" is structurally impossible in the schema (entries are owned by a book) — nothing to check |
| Static: empty/whitespace card fields the renderer skips | MET | lint/card.rs; tests lint.rs:139,153,172,186,226 |
| Model-assisted: description/personality/scenario contradictions | **MISSING** | no model-assisted lint code anywhere (grep `lint::` outside engine = none) |
| Model-assisted: description vs example-dialogue voice | **MISSING** | — |
| Model-assisted: lore contradicting card / each other | **MISSING** | — |
| Findings structured, severity-ranked, point at field/entry | MET (engine types) | `Severity`, `Scope`, `Finding`, `LintReport` server/engine/src/lint/mod.rs:41-120,335; ordering test lint.rs:877; ids test :933 |
| Suggested one-click fix | MET (engine types only) | `Fix { target, label, patch }` lint/mod.rs:~110 — patch body for ordinary update endpoint |
| Static checks: no model call, no spend | MET | pure engine module, no I/O |
| No mutation without explicit user action | MET (trivially — linter never writes; there's no caller) | lint/mod.rs:21-25 |
| Findings arrive as structured JSON the client can render (via studio/draft shape or any endpoint) | **MISSING** | no route/WS frame calls `tavern_engine::lint` (grep in server/api/src: nothing); no `lint` in api/openapi.json or api/asyncapi.md |
| Run linter over lore-extraction output before accepting | **MISSING** (note, not acceptance) | LoreExtractDialog.tsx has no lint call |
| PARITY.md updated | **MISSING** | no lint row in PARITY.md |

Clients: **neither web nor iOS.** (Hits for "lint" in apps/web are eslint comments.) The feature is invisible to users today.

---

## TAV-40 — Message classification — **PARTIAL** (Phase 1 & 2 done; Phase 3 not started)

Phases 1 and 2 in 3b9abf6 (2026-07-30, "flag repetitive replies and mark scene beats (THM-302)"); commit says "Phase 3 (emotion → portrait swap) is deliberately not attempted". iOS surfacing in 4ff66dc.

### Phase 1 — repetition (DONE)
| Requirement | Status | Evidence |
|---|---|---|
| n-gram overlap (3–5 grams, normalised) vs last N assistant turns | MET | server/engine/src/repetition.rs:22-36 (`normalize`, `MIN_N..=MAX_N`), `recent_assistant_turns` |
| Pure logic in server/engine, unit-tested | MET | repetition.rs; 35 tests in server/engine/tests/repetition.rs |
| Store `repetition_score`, flag above threshold | MET | migration server/db/migrations/0020_repetition_beats.sql (`repetition_score`, `repetition_phrase`); server publishes band `repetition_level` (ELEVATED_AT etc.); scored inline at server/api/src/ws/generate.rs:1073,1180-1199 (resolves the open question: inline) |
| Web badge + one-click regenerate via swipe path | MET | apps/web/src/components/chat/MessageParts.tsx:155-195 (`onRegenerate(message.id)`); ChatPane.tsx:590 |

### Phase 2 — beats (DONE, with a deliberate design deviation)
| Requirement | Status | Evidence |
|---|---|---|
| Extend memory.md to emit beat label + boundary flag | MET | server/api/prompts/memory.md (`[[BEAT]] label / scene: new|continues`) |
| Parser; degrade cleanly to prose-only when model ignores envelope | MET | server/engine/src/summary.rs:85 `parse`; 28 tests in engine/tests/summary.rs; memory.rs wiring in 3b9abf6 |
| Zero marginal spend (rides Memory call) | MET | same summariser call |
| Persist beats "as their own rows keyed to the message that opened them" | DEVIATION | stored as `beat_label`/`scene_start` columns on `message` (0020 migration), keyed to the first message of the window, with written rationale in the migration. "At which message" boundary precision is window-granular, not per-message |
| Prompt versioning | NOT DONE explicitly | no version marker; tolerance handled by lenient parser instead |
| Web: scrollbar markers / jump-to-beat list | PARTIAL | web renders inline `SceneMarker` dividers only (apps/web/src/components/chat/ChatPane.tsx:953,1411-1440). No scrollbar markers and no jump-to-beat list found |

### Phase 3 — emotion → portrait swap: **NOT STARTED**
No emotion classifier, no expression→image slot mapping on gallery images, no portrait swap, no new GenerationKind (GenerationKind has only Reply/Title/Memory/Draft/Speaker — generations.rs:21-35). Grep for emotion/expression across server/core, db, api: nothing.

### Acceptance
- Phase 1 unit tests + web badge: MET.
- Phase 2 clean degradation: MET.
- Phase 3 separately attributable spend: MISSING (phase absent).
- PARITY.md updated: MET for phases 1–2 (PARITY.md:62 row + footnote ⁷ at :125-133, added in 3b9abf6).

Clients: **both web and iOS** for phases 1–2 (iOS: `repetitionBadge` apps/apple/iOS/UI/ChatsView.swift:1417,1439; `SceneMarker` ChatsView.swift:819).

---

## TAV-42 — Auto-extract lorebook entries from play — **PARTIAL** (one acceptance item missing: own spend kind)

Implemented in 41d52d0 (2026-07-30, "extract lorebook entries from a chat's play (THM-304)"); iOS `LoreExtractSheet` in 4ff66dc.

| Requirement | Status | Evidence |
|---|---|---|
| New draft `kind` for lore extraction with own prompt | MET | `DraftKind::Lore` server/api/src/ws/draft.rs:50,68; server/api/prompts/lore_extract.md |
| Input: budget-clipped history (MAX_TURNS/MSG_CLIP) | MET | server/api/src/ws/lore.rs:43-45,144-146,361 |
| Input: character card(s) | MET | lore.rs `render_card` :305 |
| Input: existing lorebook entries | MET | lore.rs:125-139,165 |
| Output: keys, content, insertion_order, position, message ids | MET | prompt schema in lore_extract.md; normalised in `dedupe` lore.rs:199-262 |
| Server-side exact-key collision filter | MET | lore.rs:224 (case-insensitive, also within batch); tests lore.rs:396-425 |
| Streams like any other draft, reviewable candidates | MET | reuses draft streaming (`draft.token`) draft.rs:192-213; asyncapi.md documented |
| Manual trigger (not N-turn) | MET | web chat menu "Extract lore" ChatPane.tsx:858; LoreExtractDialog.tsx:185-187 |
| Review sheet: accept / edit / reject per candidate | MET (web); PARTIAL (iOS) | web LoreExtractDialog.tsx:229-247,346-347,455-470; iOS ManagementViews.swift:1291+ accepts one at a time, no in-place edit and no explicit reject (documented in PARITY footnote ⁶) |
| Target-book picker incl. "new book for this chat" | MET | web LoreExtractDialog.tsx:44,296; iOS ManagementViews.swift (picker, "New book for this chat") |
| No write without explicit per-candidate accept; accepted → POST /lorebooks/{id}/entries | MET | web accept() LoreExtractDialog.tsx:229; iOS `service.createEntry` ManagementViews.swift:1423 |
| Candidates cite source messages | MET (data) / weak (UI) | `message_ids` carried end-to-end; both clients only show a count ("from N messages" LoreExtractDialog.tsx:462-464; iOS "Drawn from N messages"), no link/jump to the cited messages |
| **Spend attributed under its own kind** | **MISSING** | lore drafts are recorded as `GenerationKind::Draft` with `chat_id: None` (server/api/src/ws/draft.rs:217-225); there is no `Lore`/extract kind in generations.rs:21-35. It does have its own *model* setting (`lore_model`, draft.rs:60; web SettingsPage.tsx:865-871) but not its own ledger kind |
| Linter run over extraction output | MISSING (design note, not acceptance; depends on TAV-39 having an API) | — |
| PARITY.md updated; web first then native | MET | PARITY.md:63 + footnote ⁶ (:113-123) |

Clients: **both web and iOS** (iOS reduced: no edit/reject).

---

## Summary
- **TAV-38 PARTIAL** — server + web complete. Missing: PARITY.md row, iOS speaker-mode UI, tests for the I/O fallbacks in api/ws/speaker.rs (pure fallbacks are tested). Minor: no UI for `speaker_model`, no spend-page label for `speaker`, no speculative selection (optional).
- **TAV-39 PARTIAL** — static checks fully built and tested in the engine. Missing: any API/WS surface, any client UI (web or iOS), all model-assisted checks, PARITY.md.
- **TAV-40 PARTIAL** — Phase 1 done (web+iOS), Phase 2 done (web+iOS; beats stored as message columns, not rows; web has dividers but no scrollbar markers/jump list). Phase 3 not started.
- **TAV-42 PARTIAL** — essentially done on web + iOS. Missing: spend under its own ledger kind (it's recorded as `draft`, unattributed to chat). Minor: iOS has no edit/reject; citations shown as a count only.
