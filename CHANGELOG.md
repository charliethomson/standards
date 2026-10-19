# Changelog

Notable changes to the **standards** repo: the docs, skills and templates, and the Rust
workspace in [`crates/`](crates/README.md) that serves them at https://standards.thmsn.dev and
ships the `standards` cli. Builds follow the derived versioning in
[`docs/versioning.md`](docs/versioning.md): `MAJOR.MINOR.<commit count>`, with the `vMAJOR.MINOR`
tag as the only manual knob (the first, `v0.1`, is cut at go-live). Content identity is the
commit the server embeds; entries here are human-facing.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

First draft — nothing tagged yet; everything lives here until a `v0.1` cut.

### Fixed

- **Trace → logs link** — `docs/grafana-dashboards.md`: the provisioned `tracesToLogsV2` query
  matched `service_name` from `__span.tags["service.name"]`, a resource attribute Grafana's
  span tags don't carry, so it came out empty and "Logs for this span" found nothing. It now
  matches on the trace id alone (`{service_name=~".+"} | trace_id=...`), across every service
  the trace touched.

- **`templates/rust/observability.rs`** — calls `PRODUCT_NAME.set_global()` before init (it
  exported an empty `service.name`), parses `PRODUCTION` as a bool (`is_ok()` made
  `PRODUCTION=false` production), and defaults to `Sampling::AlwaysOn` with `SAMPLE_RATE` as an
  override. Compile-checked against liblog `5af859b` (clippy pedantic clean). The compose
  template sets `OTEL_TRACES_SAMPLER=always_on` instead of `SAMPLE_RATE=1.0` (a ratio is
  parent-based and honours `sampled=0`).

### Added

- **Rust workspace + `standards-core`** (`Cargo.toml`, `crates/standards-core/`): the pure
  content model the hosted server and the `standards` cli share. Parses docs (title, sections
  with GitHub anchors, trailing `## Checklist`), archetype tables (one row per standard,
  applicability from the bold marker), skills (frontmatter, assets, description and content
  sha256), templates (strict `{{KEY}}` rendering that reports unfilled keys), the manifest,
  `.standards.conf`, the served allowlist, and absolute link rewriting. Zero I/O; tests run
  over this repo's real content.

- **`standards-db` + `standards-engine`** (`crates/`): db embeds the served allowlist
  (`docs/`, `skills/`, `templates/`, `CHANGELOG.md`) at build time and loads it once into a
  shared `Bundle` tagged with the build's commit (`RELEASE_COMMIT`, else libbuildinfo). The
  engine adds what the REST and MCP layers call: full-text search over docs, sections,
  checklist items and skills; checklists composed per standard and archetype (N/A is a value
  with the row's reason); docs and sections with absolute links; template listing and
  rendering; skills and their nested assets; the manifest.

- **`standards-api`** (`crates/standards-api/`, binary `standards-server`): the server
  skeleton. A public, read-only REST mirror at `/api/v1` (docs, checklists, templates rendered
  server-side, skills and nested assets, search, manifest) with its contract committed as
  `api/openapi.json` and a drift test; `/manifest.json`, raw templates at `/t/{path}`, Scalar
  at `/api/docs`, `/health`, `/version` (full build info), `/api/metrics` (`standards_*`),
  and `/install` (302 to the registry). Content responses carry `ETag: "<commit>"` with a
  five-minute max-age. Config is env only (`STANDARDS_BIND`, `STANDARDS_ALLOWED_HOSTS`). MCP
  and the HTML site mount next.

- **MCP at `/mcp`** (`crates/standards-api/src/mcp.rs`): rmcp 3.5 over streamable HTTP,
  mounted under poem, stateless with JSON replies and a `Host` allowlist from
  `STANDARDS_ALLOWED_HOSTS`. Tools `search`, `get_doc` (whole doc or one `section`),
  `get_checklist`, `get_template` (rendered, unfilled keys listed), `get_skill` (body plus
  asset paths), `get_skill_asset` and `version`, all answering markdown with absolute links;
  an unknown name is a tool error that lists what exists. One prompt per skill (optional
  `args`), and the server `instructions` that tell agents when to call which tool.

- **The rendered docs site** (`crates/standards-api/src/web.rs`): `/` lists the archetypes,
  each with its standards table and applicability badges, then every doc as the overview
  groups them. `/d/{id}` renders a doc with a heading TOC, self-linking anchors, wide tables
  and code that scroll in their own box, and checklists as a distinct card. Links work on any
  host. `/search?q=` renders results without JS; a small script adds live results from
  `/api/v1/search`. Light and dark follow the system; no inline script (CSP
  `script-src 'self'`).

- **`standards-cli`** (`crates/standards-cli/`, binary `standards`): the cli skeleton.
  `standards [--host URL] [--json] [-v]` with `init` (the port of `bin/standards install`
  without skill symlinks or `.mcp.json`, templates rendered by the server, never overwriting
  a file), `new <archetype> <product> [DIR]` (an empty dir, `git init`, init's files plus a
  small per-archetype set), and `self-update [--check]` (resolve from the registry,
  sha256-checked download, atomic swap of the running binary). `doctor`, `janitor` and
  `ultron` are stubs in their own modules. Exit codes 0/1/2/3 (3 = server unreachable); a
  manifest cache at `~/.cache/thmsn/standards/manifest.json` for `doctor --quiet`.

- **`docs/alerting.md`** — Grafana-provisioned alert rules as code, one header comment
  naming each rule's incident; alert on repetition and on severity or an exact message (never
  a substring a retry also logs); `noDataState` per rule; pre-seeded series; fire with runway.

- **`thmsn-logs` skill** (`skills/thmsn-logs/`) — a fleet-wide logs/metrics/traces query skill:
  `scripts/obsq.sh` queries Loki, Prometheus and Tempo through Grafana's datasource proxy
  with the read-only service-account token (`logs`, `meta`, `series`, `prom`, `trace`, `ref`,
  plus `top`, which groups WARN/ERROR lines by message for an error sweep), and ships fleet
  recipes (`problems`, `http-failures`, `latency-p95`, …) written against the standard
  field names. `{{sel}}` in a query is the repo's selector.
- **`.standards.d/<skill>/`** — per-repo specialisation of a shared skill, documented in
  `docs/skills-and-mcp.md`. `thmsn-logs` is the first user: `config` (`SERVICES`, Grafana
  overrides), `queries/` (repo recipes shadow fleet ones), `commands/` (extra
  subcommands), `NOTES.md` (the repo's fields, gaps and known noise).

- **Tests are silent** — new section in `docs/testing.md`: no test run plays sound through the
  speakers. Every audio sink goes through one gate that no-ops under test (Apple
  `TestEnvironment.isSilent`, covering XCTest, swift-testing, `swift test`, `xcodebuild test`
  and `THMSN_SILENT=1` for UI tests), Playwright launches with `--mute-audio`, Windows gates in
  `<Product>Core`. Added to the testing checklist, `AGENTS.md` and the review skill (a new
  ungated sink is always ❌).

- **`dev.thmsn.telemetry=otlp` compose label** — a service that exports logs over OTLP sets it
  on its prod compose service; the host docker log shipper drops labelled containers, so logs
  land in Loki once (pre-exporter stdout such as startup panics stays in `docker logs`).
  Documented in `docs/observability.md` (pipeline + checklist) and `docs/tracing.md` (pipeline
  hygiene); added to `templates/deploy/compose/prod.compose.yml` and the full-stack-product
  checklist.

- **Client analytics** — new `docs/client-analytics.md`: clients batch usage events to their
  own server's `POST /api/telemetry/events` (authenticated, in the contract); the server
  validates them, stamps the existing `x-<product>-*` identity and emits one `client_event`
  OTLP log per event with `event.source = "client"`; the collector routes those to
  ClickHouse (400 d, a `client_events` view) instead of Loki, and Grafana reads them back.
  Defines the event shape and limits, the reserved fleet events (`app.*`, `screen.view`,
  `error.client`, `perf.*`, `cli.command`), privacy rules (flat enum/count/duration props,
  no fingerprinting, no third-party analytics SDKs), client batching/persistence/retry, and
  the server's drop-don't-fail validation and kill switch. Linked from tracing,
  observability, the overview, the full-stack-product archetype, `AGENTS.md`, the glossary
  and the review skill.

- **Tracing** — new `docs/tracing.md`, distilled from a whole-product tracing overhaul:
  services export always-on and the collector tail-samples (errors / HTTP > 1 s / > 10 s /
  `sampling.keep` / 5 % baseline, span metrics before sampling; traces 7 d, logs 7 d);
  one span = one unit of work that finishes inside the collector's 60 s window, long work as
  linked short roots per phase, no loop/connection spans, per-item work DEBUG (with the three
  demotions that taught it); `<component>.<unit>[.<phase>]` names, `http.request` +
  `otel.name`; a levels table; 5xx-only ERROR and one log per failure; correlation ids on
  spans *and* events; privacy incl. signed URLs in `reqwest::Error` `Display` and argv; the
  client↔server contract (`traceparent` per action, `x-<product>-*` identity headers,
  `x-<product>-trace-id` + `traceId` echo, `Ref:` in client error UI); pipeline hygiene
  (`memory_limiter`, pinned images, alerting on the pipeline's own metrics, the span-metrics
  label gotcha). `observability.md` drops the 10 % `ParentBased(TraceIdRatioBased)` default for
  always-on, documents liblog's per-layer `LIBLOG_*_FILTER` env and `OTEL_TRACES_SAMPLER`, and
  the propagation helpers. `grafana-dashboards.md` gains trace ↔ log ↔ metric link
  provisioning and the `traces_span_metrics_*` series. Indexed in `AGENTS.md`, the overview,
  the full-stack-product and library archetypes and the review skill.

- **`bin/janitor` + `thmsn-janitor` skill** — sweeps `~/git` for the worktrees and cargo target
  dirs agents leave behind. `janitor scan` is read-only and gives every linked worktree a
  verdict; `janitor clean` removes only worktrees that are clean, unlocked, idle for `--days`,
  have no live process in them, and whose HEAD is merged or held by a branch. `--targets`
  clears target dirs in the worktrees it keeps, `--main-targets` idle ones in main checkouts,
  `--branches` deletes merged branches of removed worktrees.
- **No change-detector tests** — `docs/testing.md` gains a section banning tests whose
  assertions restate the implementation (mock every collaborator, verify the calls in order):
  they fail on every refactor and pass on every bug, so they are rewritten against behaviour or
  deleted, never mechanically patched. Adapted from Alex Eagle's *Change-Detector Tests
  Considered Harmful* (Google Testing on the Toilet, 2015) with the fleet-specific angle stated
  outright: coverage counts lines executed, not behaviour verified, and an agent chasing the
  80% floor will produce exactly this shape unless told not to. Gives the recognisers, the
  two settling questions, Rust (mockall) and Vitest before/after examples, what to do instead
  (assert on outputs and state; real in-memory sqlx store over mocks; stub I/O edges only;
  integration-test or exclude pure orchestration), and the carve-out for change detection that
  is the point (codegen drift checks, golden wire formats, migration checksums — pins on
  *contracts*, not implementations). Cross-linked from `AGENTS.md`, the overview index, the
  glossary, the tarpaulin template, and both audit skills.
- **READMEs** — `docs/readmes.md` + `templates/README.product.md` /
  `templates/README.library.md`: every repo keeps a root `README.md` written for a reader
  arriving cold, and so does every independently-buildable subtree. Codifies the spine the
  fleet's best READMEs already use — pitch, **mechanism** (not a feature list), layout,
  quickstart where required env vars explain *why* they're required, the commands CI runs
  plus the regeneration/drift rule, "things that will bite you", and an explicit statement
  of what is still a stub. Separates the three root docs by reader (`README.md` = what is
  this and how do I run it; `AGENTS.md` = what binds a change; `SHORTCUTS.md` = routes to
  the running system), bans hand-maintained tables of contents and captured live state
  ("prod is on 1.4.212"), and caps a README at ~200 lines with overflow promoted into
  `docs/`. Per-archetype required sections are in the doc and in each archetype checklist.
  `standards install` scaffolds the archetype-appropriate skeleton and `standards sync`
  backfills it (neither clobbers an existing README); the maintenance rule — change the
  behaviour, update the README in the same commit — lives in the consuming repo's
  `AGENTS.md` stub.
- **Shortcuts** — `docs/shortcuts.md` + `templates/SHORTCUTS.md`: every repo keeps a root
  `SHORTCUTS.md` answering the operational questions that get asked repeatedly ("is it up to
  date in prod?") with one command each, so agents stop re-deriving prod hosts and deploy
  mechanisms from workflows and compose files every session. The rule is read-before-recon /
  write-after-recon, recording the **route** (endpoint, command, mechanism) rather than the
  **reading** (a version number, a timestamp) so entries don't rot; entries carry a
  `_Verified:_` date and a wrong one gets fixed on the spot. The trigger lives in the
  consuming repo's root `AGENTS.md` stub (what agents actually load); `standards install`
  scaffolds the file and `standards sync` backfills it into repos installed earlier
  (neither clobbers an existing one).
- **Shared web component library** — `docs/web-architecture.md` now mandates consuming the
  fleet's **`@thmsn/ui`** package (private registry `npm.dev.thmsn.dev`; shadcn/Radix + HSL
  tokens) instead of re-vendoring shadcn per app, and documents both Tailwind consumption
  paths: **v4 CSS-first** via `@thmsn/ui/theme.css` (recommended for new and migrating apps;
  no `tailwind.config`) and **v3** via `@thmsn/ui/tailwind-preset` + `@thmsn/ui/tokens.css`
  (legacy). Product rebranding is identical on both — a generated `tokens.css` (same
  variable names, `:root` + `.dark`) imported after the library's theme/tokens;
  `docs/branding.md` notes the override. New prompt `prompts/migrate-to-thmsn-ui.md` walks
  an app through an incremental migration.
- **Entrypoint & index** — `README.md`, `AGENTS.md` (agent entrypoint), `docs/overview.md`
  (operating context + archetype picker + index).
- **Archetypes** — `docs/archetypes/`: full-stack-product, library, orchestration, cli-tool,
  vendored-app, each mapping which standards apply and how strictly.
- **Foundational standards** — versioning, identifiers, build-info, ci-cd, self-hosted-agents,
  deployment, registry-publishing, testing, platform-ux, workflow, branding, skills-and-mcp.
- **Implementation standards** (services & clients) — service-architecture, data-persistence,
  contracts, error-handling, auth-integration, configuration, observability, security,
  web-architecture, rust-conventions, lib-ecosystem.
- **Grafana dashboards** — `docs/grafana-dashboards.md`: dashboards are committed
  `<name>.dashboard.json` next to their stack (author in UI → export → commit); queries key
  off fleet identifiers + structured log fields; export hygiene (strip `id`/`version`/
  `iteration`), datasource-uid pinning, and a provision-dashboards next step. Modelled on
  the fleet's real overview dashboards.
- **Public short ids** — `docs/public-ids.md` + `templates/rust/public_id.rs`: user-facing
  entities carry an 11-char Crockford Base32 alias (a stored `public_id`, not a UUID encoding)
  translated to/from the internal `Id<T>` in the exposer layer; insert retries on a
  `public_id` collision (distinct from domain conflicts), ordering/pagination stays on
  `Id<T>`; cross-linked from data-persistence, contracts, identifiers, and overview.
- **Glossary** — `docs/glossary.md`: conventions, concepts, infrastructure, and the `lib*`
  toolchain defined in one place.
- **Prompts** — `prompts/`: copy-paste prompts for agents — `install.md` (wire up the
  submodule) and `validate-install.md` (check an install is correct + committed). `install` now
  reminds which files to commit (including the `.claude/skills/` symlinks).
- **Copyable templates** (`templates/`) — `VERSIONING.md`; `rust/` (`build.rs`,
  `tarpaulin.toml`, `id.rs`, `public_id.rs`, `error.rs`, `config.rs`, `observability.rs`,
  `Cargo-workspace-lints.toml`); `github-workflows/` (`ci.yml`, `server.build.yml`,
  `web.build.yml`); `docker/Dockerfile.rust`; `deploy/` (compose, Caddyfile, komodo sync);
  `branding/build.py`; the consuming-repo `AGENTS.md` stub; `link-standards.sh`; `mcp.json`.
- **Shared tooling scaffolding** — `skills/` and `mcp/` with conventions; `link-standards.sh`
  symlinks shared skills into `.claude/skills/` and scaffolds `.mcp.json`.
- **Shared skills** — six `thmsn-*` audit/scaffold skills: `thmsn-standards` (audit/fix the repo against its
  applicable standards), `-review` (diff-scoped check of just your changes), `-init` (scaffold a
  new repo to an archetype), `thmsn-new-component` (add a service or client surface on-standard),
  `-sync` (update the submodule + re-audit), and `-contribute` (author + push a genericized
  standards change). Consuming repos get them via the submodule + `link-standards.sh`.
- **`thmsn-jarvis` + `thmsn-ultron` skills** — a two-halves workflow for programs of work, generalized
  from the ad-hoc prompts used for the auth-remediation and compact-view programs.
  **Jarvis plans**: refines a rough idea *with the user before researching*, fans out spec agents per
  workstream and reconciles them, surfaces the decisions that are the user's to make (each with a
  recommendation), then files discrete Linear tasks with real blocking relations plus a program brief.
  **Ultron executes**: reads that backlog, sequences around the blocking chains, dispatches
  self-contained implementer sub-agents, verifies their output before marking anything Done, and
  escalates human-gated actions (cutovers, secrets, ordering hazards, identity calls). Jarvis's output
  *is* Ultron's input; Ultron handed an unscoped idea points back at Jarvis rather than inventing a
  decomposition.
  Programs are **pausable and resumable across sessions** — state splits by durability: the plan,
  decisions and blockers go to Linear (readable from a phone while a usage limit resets), while
  volatile progress lives at `~/.local/state/thmsn/ultron/<program-slug>/` as a `PROGRAM.md` ledger
  plus a per-task journal each worker updates continuously. A worker killed mid-step resumes from its
  first unchecked step rather than restarting. The slug is derived (`<team>-<label>`), never invented,
  so a fresh session can find the program cold.
- **`bin/ultron`** — read-only CLI over that state (`ls`, `status`, `watch`, `log`, `gc`), so checking
  where a program stands costs no tokens. `watch` is a live view that redraws only when the state
  actually changes, waits for a program to appear rather than erroring if you start it first, and
  prints once instead of emitting escape codes when its output isn't a terminal. Symlink to
  `~/.local/bin/ultron`.
- **Integration CLI** (`bin/standards`) — `install` (wire the submodule into a repo), `sync`
  (pull upstream + show the changelog + re-link + stage the bump), `contribute` (genericize a
  repo's identifiers and push the change upstream), and `lint` (verify your edits carry none of
  your product's own names). Identity is read from a consumer-side `.standards.conf` — **no
  product names are stored in this repo**.

### Changed

- **Tracing and logging, from a whole-product audit** (`docs/tracing.md`, `docs/observability.md`,
  `docs/error-handling.md`):
  - The server **adopts** an app/browser `traceparent` (`liblog::adopt_client_trace`: same
    trace id, no parent, a link) and **continues** only a fleet service's (`server_span!`);
    continuing a client trace left every client trace rootless in Tempo.
  - Every request leaves exactly one `http completed` line with a fixed field set, errors
    included; a 5xx's mapper records `error.type` on the request span through a
    request-scoped handle instead of logging, so a 5xx logs once. Marking a span ERROR and
    logging the failure are separate duties.
  - 5xx bodies are JSON `{message, $type, traceId}` with fixed client text; `to_poem` no
    longer returns `err.to_string()` (it leaked SQL and upstream bodies).
  - Outbound calls are one `client_span!` per wire call with `peer.service` /
    `db.system` + `server.address`; wrappers and SDK-wrapping spans are INTERNAL; a hot
    route a service calls exports at the caller's level so the service graph
    pairs. The old `auth.me` example was the wrong kind.
  - New span-shape rules: unbounded waits closed at 55 s, `*_wait_ms`/`acquire_ms` for dead
    time, `Span` vs `SpanContext` across queues (no stored clones), a `<service>.startup`
    root, connections as a connect span + one close event, cache hits DEBUG, outbound body
    reads as a sibling span.
  - WARN means recovered (retries DEBUG until the outcome); libraries don't log errors they
    return; no silent `.ok()`; a panic hook for background tasks; fatal boot errors logged
    before `main` returns; retried calls record `first_attempt_ms`/`first_error`.
  - One `outcome` vocabulary, durations always `_ms`, late fields declared `Empty`,
    filterable fields flat beside any `Valuable`.
  - `NOISY_TARGETS` listed correctly; an env filter override replaces it wholesale (the old
    `info,somecrate=debug` example re-enabled `h2`'s connection spans).
  - Propagation lists the liblog helpers per situation. Pipeline: `OTEL_BSP_MAX_QUEUE_SIZE=8192`,
    Tempo's 5 MB trace cap and 168 h search window, servicegraph expired edges.
  - Metrics: HTTP metrics from the outermost layer with bounded labels, `_build_info` and
    `_process_start_time_seconds`, pre-seeded label values, a render test.
- **`docs/grafana-dashboards.md` Delivery** describes provisioning as it runs (product
  dashboards bind-mounted from the stack checkout; mirrors need a drift check).

- **Client analytics, settled by the first adoption** — `docs/client-analytics.md`: the ingest
  route is `POST <api base>/telemetry/events`, under the product's API base (`/api/v1/...`),
  still in the contract and behind normal auth. The handler lives in the private
  `libanalytics` (products mount the route, supply identity + the auth user, and record the
  drop metric from the per-reason counts it returns) instead of "copy it until the second
  product"; the 400 body is a fixed string, and any non-401/403 4xx from the body-cap/parse
  layer counts as `reason="batch"`. Clients drop a batch on any 4xx **except 401**, which
  refreshes auth and keeps the queue (web pauses sending while auth fails); native queue lines
  carry the identity they were recorded under and flush in per-identity groups, and the native
  queue is cleared on logout; the 60 KiB cap applies to every web batch. The one-recorder rule
  points at `libanalytics`' Rust (`recorder` feature), Swift (SSH git URL + semver tags;
  `-scmProvider system` in CI) and TS (`@thmsn/analytics` on Verdaccio) recorders, with the
  product supplying the transport and auth state; WinUI stays per product. Install/session
  ids are UUIDv4, matching tracing. `lib-ecosystem.md` lists `libanalytics`; the route path is
  updated in tracing, the glossary and the full-stack-product archetype.

- **Public ids may live on models and rows** — `docs/public-ids.md` said `core`/`db`/`engine`
  never learn an entity has a public id, while the same doc has `db` mint it at insert and own
  the resolver. Now: models and row structs may carry `public_id`, exposed event/audit rows may
  cite public ids, internal calls/FKs/cursors stay on `Id<T>`, and inbound resolution plus the
  choice of wire id stay in the exposer layer. Checklist item amended to match.
- **Tracker moved from Linear to vision** — `thmsn-jarvis`, `thmsn-ultron` and
  `thmsn-deep-review` now drive the self-hosted **vision** tracker through the `vision` CLI and
  point at the `vision` skill for flags. The model maps directly: one vision project per repo
  (found by matching its `repo`, created with a 2–8 character slug when missing) replaces
  team + project; a cross-repo program is a plain kebab label (`auth-remediation`, no team-key
  prefix), since a parent can't cross projects; Bug/Feature labels become the workitem `kind`.
  Ultron's program slug is now derived from that label (or a parent ref), and workers still
  comment at boundaries only but also attach commits and PRs with `vision issues link`. With no
  bulk-create, Jarvis and deep-review file one `vision issues create` per task, stop on the first
  failure, and count the result with `vision issues list --label … --all` — the same
  don't-trust-a-half-filed-backlog guard that `failedCount` used to be. Agents act under their
  own `vsn_` key so the timeline attributes to them. `prompts/linear-project-from-codebase.md`
  is now `prompts/vision-project-from-codebase.md`.
- **`docs/observability.md`: a component whose stdout is a wire logs to stderr.** The mandate
  to initialise `liblog` was unconditional, and `liblog` only knew how to write its
  human-readable layer to stdout — so a stdio-protocol server (an MCP server speaking JSON-RPC
  over stdin/stdout) could not comply without corrupting its own transport on the first
  `tracing::info!`, and a standards audit would have flagged it for correctly not complying.
  The carve-out is stated in the **Rule** itself, with the mechanism in a new *When stdout is
  the wire* section: `liblog` stays mandatory, initialised with
  `.console_stream(ConsoleStream::Stderr)` (the redirect added to `liblog` alongside this) —
  `.quiet(true)` is not the fix, since it protects the wire by giving up logs. The section also
  states the corollary that nothing else may write to stdout either, and that this is proven by
  **mutation** — inject one `println!` and watch the protocol tests fail — because inspection
  cannot prove the absence of a stray write. Checklist item added; the "fans out to pretty
  stdout" description is now "console (stdout by default, redirectable)".
- **`product_name!` alone does not set the identifier — the docs now say so.** The macro only
  *declares* a const; without a `set_global()` call `ProductName::global()` stays `None`, the
  OTel `service.name` falls back, and every `libpath`-derived root (logs, config, cache)
  silently lands under `dev.thmsn.unspecified`. The old wording ("`product_name!(…)` **set**
  before init") read as "declared" to anyone who had just typed the macro, and components
  following it in good faith got it wrong. Corrected in `docs/observability.md` (bootstrap
  snippet + checklist), `docs/identifiers.md` (new *Declaring is not publishing* section, the
  "How to set it" table, and the checklist), `docs/archetypes/cli-tool.md`,
  `docs/rust-conventions.md`, and `templates/rust/observability.rs` — which scaffolded the
  incomplete form and so propagated it. The word *set* is avoided throughout in favour of
  **declared and published**. All snippets bind the const to a local before calling
  `set_global()`, since `product_name!` expands to a `const` holding a `LazyLock` and borrowing
  it inline both acts on a fresh copy and trips `clippy::borrow_interior_mutable_const`.
- **Typed identifiers moved to `libid`** — `Id<T>` and `PublicId<T>` are now consumed from the
  shared **libid** crate (git dep, features `serde`/`sqlx`/`poem-openapi`) instead of copy-pasted
  templates; `templates/rust/id.rs` and `templates/rust/public_id.rs` are removed.
  `docs/public-ids.md` documents the `PublicEntity` trait: default is the bare 11-char code, an
  entity may opt into a self-describing prefix (`ord_9TXK4P2RQ8M`) that becomes part of the
  canonical (stored/displayed/parsed) form. libid added to the `lib*` catalogue and the
  workspace-deps template.
- Replaced the reconstructed `templates/VERSIONING.md` with the byte-identical fleet canonical.
- Standardized GitHub Actions naming on **Scheme B** (component-first dot-namespaced):
  renamed `build-server.yml`/`build-web.yml` → `server.build.yml`/`web.build.yml`; added the
  "Workflow naming" section to `docs/ci-cd.md`; updated all references.
- `docs/configuration.md`: **`libconfig`** is now the fleet config tool for services too — after
  landing the `libconfig` shared-env layer + `Loader` file source (`2173a3a`) and the `libpath`
  base-dir override + no-create (`251628a`). Removed the interim `figment` framing; the
  `config.rs` template now uses `Loader::path(..).env_prefix(..).shared_env(..)`.
- `docs/identifiers.md`: reframed the segment after `<product>` as a **`<component>`** (a
  service like `server`/`worker`/`cli` or an app surface like `ios`/`macos`/`winui`/`web`),
  with grants documented as a parallel axis and an explicit grammar — replacing the looser
  `<platform|grant>` shorthand.
- **Anonymized examples** across docs + templates: product/library names → `dev.thmsn.someproduct`
  / `libsomeproduct`, and the sample domain vocabulary → neutral terms (resource/model names, an
  external-adapter example, a metric name) so the docs are fully product-neutral. The real `lib*`
  toolchain and shared-infrastructure names are kept — they're the actual platform, not examples.

- `docs/error-handling.md`: formalized **reverse-DNS error keys** — every serializable error
  variant carries a stable `$type` key `dev.thmsn.<root>.<area>.error.<kind>`, specific to the
  code path, via `#[serde(rename = …)]` (libs drop the `lib` prefix from the root). Extended the
  `error.rs` template, added a glossary entry, and cross-linked from `identifiers.md`.

### Notes

- Standards are written **canonically** (the rule as target state), per the deep-docs decision.
- Known external follow-up: a reference repo's `VERSIONING.md` has drifted from the fleet canonical.
