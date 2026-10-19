# crates

**The code that serves these standards over HTTP and MCP, and the `standards` cli.**

One Cargo workspace (root [`Cargo.toml`](../Cargo.toml)). The server embeds this repo's
`docs/`, `skills/`, `templates/` and `CHANGELOG.md` from its own commit and serves them at
https://standards.thmsn.dev as rendered docs, a REST mirror and an MCP endpoint. The cli
installs per machine and keeps skills, the MCP registration and each repo's setup in step
with that server.

## How it works

```
repo files ──(allowlist)──▶ standards-db ──bytes──▶ standards-core ──▶ Bundle
                                                                        │
                    standards-engine (search, composed checklists) ◀────┘
                                   │
                    standards-api (HTML /d, REST /api/v1, /mcp, /manifest.json)

standards-cli ──REST──▶ standards-api        standards-cli ──depends on──▶ standards-core
```

`standards-core` is the shared model and does no I/O: callers hand
`Bundle::from_files` pairs of `(repo-relative path, bytes)` plus a commit and get back
parsed docs, archetypes, skills and templates. Everything the server publishes is derived
from that bundle, so content identity is the commit and binary identity is the derived
version of the same commit.

Properties worth knowing before you change anything:

- **The allowlist is in core** (`is_served`): `docs/**`, `skills/**`, `templates/**`,
  `CHANGELOG.md`, never dotfiles. The db crate's embed patterns match it, and every embedded
  path is checked with it again; a test proves the two agree.
- **Content identity is the commit.** `standards_db::commit()` is `RELEASE_COMMIT` at build
  time when CI sets it, else the commit libbuildinfo recorded for this build (the same commit
  the derived version counts to), else `unknown`. A local build from a dirty tree reports
  its HEAD commit.
- **Search is in memory.** The engine indexes doc titles and intros, each section's own
  text, each checklist item and each skill's name and description when it is built, and
  scores with BM25 plus heading and doc-title boosts. Hits point at a doc id and section
  anchor (or a skill name).
- **Links are rewritten, not stored absolute.** Docs keep relative links so they work on
  GitHub; `Linker::rewrite` points them at `/d/<id>#anchor`, `/t/<path>` or GitHub at the
  commit when served.
- **Template placeholders are strictly `{{[A-Z_]+}}`.** Any other braces (Python f-strings,
  free-text prompts in the deep-review asset) are content and survive rendering.

## Layout

| Crate | Role |
|---|---|
| [`standards-core`](standards-core/) | Content model and parsers: `Bundle`, `Doc`, `Checklist`, `Archetype`, `Skill`, `Template`, `Manifest`, `StandardsConf`, link rewriting, `MIN_CLI_VERSION`. |
| [`standards-db`](standards-db/) | The served files embedded at build time (rust-embed, one folder per served dir, plus `CHANGELOG.md`), re-checked with `is_served`; `files()`, `commit()`, and `bundle()`, which parses once and hands out an `Arc<Bundle>`. |
| [`standards-engine`](standards-engine/) | `Engine` over an `Arc<Bundle>`: `search`, `checklist(standard, archetype)`, `doc` / `doc_section` with absolute links, `templates` / `render_template`, `skills` / `skill` / `skill_asset`, `manifest`. Typed errors for unknown names; N/A is a value. |

| [`standards-api`](standards-api/) | The server binary `standards-server` (`dev.thmsn.standards.server`): `app()` is the whole mount map; REST mirror (`rest.rs`), wire types (`dto.rs`), engine errors as keyed 404s (`error.rs`), ETag caching (`caching.rs`), slash-holding path parameters (`paths.rs`), the request layer and metrics, env config. MCP at `/mcp` (`mcp.rs`); `web.rs` is the mount point for the HTML site. |

`standards-cli` joins the workspace when it is built.

### The server's routes

| Route | Serves |
|---|---|
| `GET /api/v1/docs`, `/api/v1/docs/{id}` | doc list; one doc's markdown (links absolute) and outline |
| `GET /api/v1/checklist?standard&archetype` | composed checklist; N/A is a 200 with `applicability: not_applicable` |
| `GET /api/v1/templates`, `/api/v1/templates/{path}?KEY=V..` | template list; a render with `body` and `unfilled` |
| `GET /api/v1/skills`, `/api/v1/skills/{name}`, `/api/v1/skills/{name}/assets/{path}` | skill list; one skill with hashes; an asset's bytes |
| `GET /api/v1/search?q&limit` | hits over docs, sections, checklist items, skills (`limit` 1..50, default 10) |
| `GET /api/v1/manifest`, `/manifest.json` | the manifest (the same document) |
| `GET /t/{path}` | a raw template, `text/plain` |
| `GET /openapi.json`, `/api/docs` | the contract; Scalar |
| `GET /health`, `/version`, `/api/metrics` | liveness; derived version, commit and full libbuildinfo; OpenMetrics |
| `GET /install` | 302 to `https://apps.dev.thmsn.dev/install/dev.thmsn.standards` |
| `POST /mcp` | MCP, streamable HTTP (`mcp.rs`; see below) |
| `/`, `/d/{id}` | the HTML site (STD-17, `web.rs`) |

- **Ids and paths with slashes are one parameter.** `{id}`, the template `{path}` and the
  asset `{path}` may hold slashes (`archetypes/cli-tool`, `rust/build.rs`,
  `scripts/obsq.sh`). Send them percent-encoded (`/api/v1/docs/archetypes%2Fcli-tool`, which
  generated clients do) or raw (`/api/v1/docs/archetypes/cli-tool`): `paths.rs` encodes the
  tail of a raw-slash request before routing, so both reach the same handler.
- **Misses are 404s** with `{"message", "$type", "traceId"}`, `$type` one of
  `dev.thmsn.standards.server.content.error.unknown_{doc,section,archetype,skill,asset,template}`.
- **Caching:** content routes (`/api/v1/*`, `/manifest.json`, `/t/*`) send
  `ETag: "<commit>"` and `Cache-Control: public, max-age=300`, and answer a matching
  `If-None-Match` with 304. A build whose commit is `unknown` or whose tree was dirty sends
  neither, since its commit does not pin the bytes.
- **Config** is env only (`Loader::pure_env()`): `STANDARDS_BIND` (default
  `127.0.0.1:8080`), `STANDARDS_ALLOWED_HOSTS` (the Host allowlist `/mcp` uses, default
  `standards.thmsn.dev,localhost,127.0.0.1,::1`), and the shared `OTLP_ENDPOINT`,
  `PRODUCTION`, `SAMPLE_RATE`.
- **Skill bodies are served as written** (not link-rewritten), so `body` hashes to what the
  manifest's `content_sha256` covers.
- Logs carry the path, never the query string: search terms and template values stay out.
- **MCP** (`mcp.rs`) is rmcp's `StreamableHttpService` mounted with poem's tower-compat,
  stateless (no `mcp-session-id`, `GET /mcp` is 405) with `application/json` replies. A
  request whose `Host` is not in `STANDARDS_ALLOWED_HOSTS` gets a 403, so the public hostname
  must stay on that list. Tools: `search{query, limit?}`, `get_doc{id, section?}`,
  `get_checklist{standard, archetype}`, `get_template{path, vars?}`, `get_skill{name}`,
  `get_skill_asset{name, path}`, `version{}`; each answers markdown, and a miss is a tool
  error (`isError: true`) naming what exists. One prompt per skill, with an optional `args`
  appended to the body. The `instructions` string is `mcp::INSTRUCTIONS`. rmcp logs request
  params only at DEBUG, so never run it with `rmcp=debug` where search text matters.

Run it locally:

```sh
cargo run -p standards-api --bin standards-server          # serves 127.0.0.1:8080
STANDARDS_BIND=127.0.0.1:8099 cargo run -p standards-api --bin standards-server
curl -s localhost:8080/manifest.json
# MCP from Claude Code, local scope (remove it afterwards with `claude mcp remove -s local standards-dev`):
claude mcp add --transport http --scope local standards-dev http://127.0.0.1:8080/mcp
```

## Development

These are the gates CI runs once its pipelines land; passing them locally means passing CI.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo tarpaulin            # tarpaulin.toml: fail-under = 80
```

`cargo llvm-cov --workspace --summary-only` gives the same number locally where tarpaulin's
profile parser lags the toolchain (macOS).

The REST contract is committed at [`api/openapi.json`](../api/openapi.json) and
`cargo test -p standards-api` fails when it differs from what the server emits. After
changing a route or a wire type, regenerate it:

```sh
cargo run -q -p standards-api --bin standards-server -- openapi > api/openapi.json
```

## Things that will bite you

- **Core's tests read the real repo content.** A doc edit that drops a `## Checklist`, adds
  a link to a doc that doesn't exist, or breaks an archetype table row fails
  `cargo test -p standards-core`. Fix the doc, not the test.
- **Content is compiled in.** The server serves what it was built with; `standards-db`'s
  `build.rs` reruns when `docs/`, `skills/`, `templates/` or `CHANGELOG.md` change, so a
  rebuild picks up edits. Its tests fail if a served file on disk isn't embedded or an
  unserved one is.
- **`MIN_CLI_VERSION` is hand-bumped.** Change the manifest or REST shape in a way an older
  cli would misread and you bump it in the same commit.
- **A new route outside `/api/v1` needs a name in `request.rs`** (`api_operation`), or its
  requests log and count as `unmatched`.

## Status

`standards-core`, `standards-db`, `standards-engine` and `standards-api` (REST mirror,
MCP at `/mcp` and the standard mounts) exist. The HTML site and the cli are not built yet,
and nothing is deployed.

Conventions for this code: the "This repo's own code" section of [`AGENTS.md`](../AGENTS.md).
