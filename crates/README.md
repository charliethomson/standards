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

`standards-api` and `standards-cli` join the workspace as they are built.

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

## Status

`standards-core`, `standards-db` and `standards-engine` exist. The api (REST, MCP, HTML)
and the cli are not built yet, and nothing is deployed.

Conventions for this code: the "This repo's own code" section of [`AGENTS.md`](../AGENTS.md).
