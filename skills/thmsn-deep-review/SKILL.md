---
name: thmsn-deep-review
description: Deep, whole-repo code review that reads the source (not just the diff), one module at a time, and writes a single dated markdown report — reviews/review-YYYY-MM-DD.md — pairing the overarching review with the findings, then optionally files the recommendations as prioritized, self-contained Linear tasks and lists them in the report. Use for "review the whole repo / every module", "audit this codebase", "deep code review", or "review X and file the findings as Linear tasks". NOT for diff-scoped checks (use /thmsn-standards-review or /code-review).
---

# thmsn-deep-review — whole-repo audit → one markdown report → Linear tasks

Produces a **module-by-module** audit of an entire repo across four dimensions —
**correctness & bugs, security, architecture & maintainability, testing & CI** — as a single
dated markdown file, `reviews/review-YYYY-MM-DD.md`. Optionally files the recommendations as
prioritized Linear tasks and lists them in the report.

This is a **deep** review: subagents read the actual source and cite `file:line`. It is not
the diff-scoped `/thmsn-standards-review`, nor the correctness-of-a-diff `/code-review`.

## How it works

One deep-audit **subagent per module, in parallel**. Each agent reads its module and *returns*
its findings as a markdown section (it writes no files). You then compose them into one
document: an overarching review up top (scorecard, architecture, cross-cutting themes,
prioritized fixes) followed by the per-module sections, and — if asked — a list of the Linear
issues created. The parallelism is what makes a whole-repo audit tractable; a single markdown
file keeps it portable, diffable, and committable.

Bundled asset: [`assets/review.template.md`](assets/review.template.md) — the exact document
skeleton (front-matter summary, scorecard table, architecture, cross-cutting themes, "what to
fix first", per-module sections, issues-created list, methodology). Fill it in; don't reinvent
the shape.

**One file, dated.** Write to `reviews/review-YYYY-MM-DD.md` (get the date from
`date +%F`). The date in the name lets successive reviews coexist and be committed as a
history. Create the dir with `mkdir -p reviews`.

---

## Phase 0 — Ask, then scope

**Ask 2–4 clarifying questions first** (the user usually expects this for a broad review):
depth (overview vs architectural vs deep line-level), which dimensions to emphasize, audience/
tone (author = blunt/technical; contributor = explanatory; stakeholder = polished), and whether
to also create Linear tasks. Default to author-tone + all four dimensions if they decline.

Then enumerate the modules. For a `full-stack-product` archetype that's typically:

| Kind | Where | Notes |
|---|---|---|
| Rust workspace crates | `server/*` (`core`, `db`, `<domain>`, `engine`, `api`) | one section each |
| API contract | `api/openapi.json` + `api/asyncapi.md` | contract-level; enumerate structurally, don't read 3k lines linearly |
| Client apps | `apps/web`, `apps/apple` (iOS+macOS+shared kit), `apps/windows` | one section per surface |
| Tools / CLIs | `tools/*`, `bin/*` | skip empty stubs — note them in the report |

Size each module (`find … -name '*.rs' | xargs wc -l`) to calibrate effort and fill the
scorecard. **Flag generated code** (Kiota Swift/C# clients, `openapi.json`, `Generated/` dirs)
— note it exists and audit how it's *wrapped/used*, but don't line-audit generated output.

## Phase 1 — Dispatch one deep-audit agent per module (parallel)

Launch the agents in a **single message** so they run concurrently (general-purpose agent).
Each prompt must contain:

1. **Repo root + module scope** — exact paths and which files to read in full.
2. **The four dimensions**, with a *module-tailored security emphasis*:
   - `db` → auth/tenant scoping, SQL injection (dynamic SQL), transaction boundaries, lost updates.
   - `api`/service → is every route behind auth? unauthenticated endpoints? admin-grant enforcement? SSRF on proxy routes? secret/`?token=` leakage in logs/errors; rate limiting; body-size caps.
   - `engine`/import → SSRF on user-supplied URLs (private-IP/redirect/size limits), decompression bombs, malformed-input panics (`unwrap`/slicing), path traversal.
   - web → XSS in markdown/AI-content rendering, token storage (localStorage vs memory), CSP, secrets in bundle.
   - native clients → credential storage (Keychain/DPAPI vs plaintext), ATS/HTTPS enforcement, token-refresh replay, WS streaming lifecycle, unsafe force-unwraps.
   - contract → is a security scheme defined & applied? `nullable`/tri-state modeled? drift-check in CI?
3. **Severity rubric** (below) and an instruction to include genuine **"What's good"** notes — verified, not assumed.
4. **Tone**: match what Phase 0 established (default: blunt, technical, for the author).
5. **Return format — the agent writes NO files.** It returns a self-contained markdown section for its module, ready to drop into the report, in exactly this shape:

   ```markdown
   ### <module>

   **Grade: <A–F>** · <lang> · <LOC> LOC · <one-line description>.

   <2–3 sentence summary: role, health, main takeaway.>

   - **[High]** <title> — `path/file.rs:LINE` — <what's wrong, the failure scenario>. Fix: <fix>.
   - **[Med]** … — `file:LINE` — … Fix: …

   **What's good:** <verified strengths>.

   **Testing & CI:** <coverage state, gaps, what to add>.
   ```
   Findings **most-severe first**; severity tag is one of `[Critical] [High] [Med] [Low] [Nit]`.
6. **Also return a one-line scorecard row**: `| <module> | <lang> | <LOC> | <grade> | c/h/m/l/n | <top risk> |`.
7. "Actually READ the source — do not guess. Cite `path/file:line`. Don't ask me questions — make judgement calls."

Feed agents any relevant repo docs (`SECURITY.md`, `PLAN.md`, parity docs, `standards/docs/*`)
and known gotchas so they don't re-derive them.

## Phase 2 — Compose `reviews/review-YYYY-MM-DD.md`

Assemble the returned sections into the bundled template. Write these parts yourself from the
agents' summaries:

- **Header + totals** — overall grade, module count, total findings, per-severity counts.
- **Executive summary** — the single most important takeaway; strongest/weakest parts; anything "fix now".
- **Scorecard table** — paste the agents' scorecard rows; link each `Module` cell to its section anchor.
- **Architecture** — a fenced ASCII map of how the modules relate (sources of truth, data flow) + a note on generated-vs-hand-written and what was/wasn't line-audited.
- **Cross-cutting themes** — the most valuable section. The *same root cause* recurring across ≥2 modules (e.g. one token-refresh bug in every client, spec drift flagged by both api and contract, a streaming-reset gap in web+apple+windows). Name every module each touches and give the single upstream fix.
- **What to fix first** — a prioritized P0…P3 table (fix / where / why-now), one row per high-leverage item.
- **Module reviews** — the agents' sections, in dependency order (foundations first).
- **Methodology footer** — what was and wasn't line-audited; the severity legend.

## Phase 3 — (Optional) Prioritized Linear tasks

Only if the user asked. Use the **`linear`** CLI (see the `linear` skill for full reference).

1. **Discover context**: `linear teams list`; find the project in `linear projects list`; get
   states + labels. Team keys work anywhere a `--team` is taken, so `linear states list --team ENG`
   is fine. Create missing labels (`Security`, `Testing`, …) — note a label like `Bug` may already
   exist as a *workspace* label (not in the team list); resolve its id from `linear labels list`
   (no `--team`).
2. **One task per actionable recommendation** — consolidate nits/lows into per-module "hardening
   roundup" tasks so the list stays high-signal (~20–35 total for a full repo). Each task is
   **self-contained**:
   > **Context** (module + role) · **Problem** (`file:line`, the concrete failure) ·
   > **Fix** (steps) · **Acceptance criteria** (checkboxes) · **Source** (this review file).
3. **Priority = severity**: `1` urgent (Critical + the scariest High), `2` high, `3` medium,
   `4` low. Put priority 1–2 in **Todo**, 3–4 in **Backlog**.
4. **Bulk-create**: write the array to a temp JSON file and `linear issues bulk-create --file …`.
   Prefer a small Python generator that holds the descriptions and emits the JSON.
   **Check `failedCount` in the output, not the exit code** — a partial failure still exits 0,
   so a silently half-filed backlog looks identical to a complete one.
5. **Wire systemic dependencies** with relations. Issue identifiers (`ABC-123`) work on both
   `--issue` and `--related`, so you can wire straight from the report without mapping
   identifier→id. Use `blocks` for real ordering (e.g. "server emits terminal frame" blocks the
   per-client resets) and `related` for siblings.
6. **Record them in the report** — add the "Linear issues created" section: group by priority,
   one line each (`ABC-154` [title](url) — labels), plus the relations. Then thread the issue
   refs back into the per-module findings and the "what to fix first" table (append `→ ABC-154`)
   so the review and the backlog point at each other.

---

## Severity rubric

| Severity | Meaning |
|---|---|
| **Critical** | Exploitable, data-loss, or crashes in prod; auth bypass; RCE; SSRF hitting internal metadata |
| **High** | A real bug or security gap that will bite under normal use |
| **Medium** | Likely bug or notable design flaw |
| **Low** | Minor issue; correctness-preserving |
| **Nit** | Style / cosmetic |

Every module section ends with genuine **What's good** notes — a review that only lists faults
is untrustworthy.

## Conventions & guardrails

- **The report is a single file: `reviews/review-YYYY-MM-DD.md`.** Dated so snapshots coexist;
  commit them as a history if you want (ask first). Don't emit per-module files or HTML.
- **Don't commit** the report or the tasks unless asked (repo *or* the standards submodule).
- **Generated code is noted, not line-audited.** Say so in the methodology footer.
- **Verify before claiming done** — every `file:line` an agent cites should be real (spot-check
  a few), and internal section links should resolve. Don't overstate: if a module was skimmed,
  say so.
- Keep the cross-cutting section honest — a "systemic" theme needs the *same* root cause in ≥2
  modules, not two superficially similar findings.
