# Archetype: Library

A shared Rust crate consumed by other repos over git. Lighter than a product: no deploy,
no clients, often no CI. The emphasis is a clean public API and being a good dependency.

Reference: `libsomeproduct` (a domain library), plus the shared `lib*` toolchain —
`liberror`, `liblog`, `libsignal`, `libconfig`, `libpath`, `libbuildinfo`, `libcmd`, `libwhich`, ….

## Repo shape

Minimal (single crate) or a workspace when there are optional platform backends:

```
lib<name>/
├── Cargo.toml              workspace root; one unified placeholder version
├── lib<name>/              the public crate (or src/ for a single-crate repo)
├── core/common/            shared traits/types          (only if multi-crate)
├── platform/<backend>/     feature-gated backends       (only if multi-crate)
├── examples/               executable usage = integration tests + API docs
├── .cargo/config.toml      if it needs tracing/valuable or git-fetch-with-cli
├── README.md               the primary API documentation
└── standards/              this submodule
```

## Standards that apply

| Standard | How it applies here |
|---|---|
| [Versioning](../versioning.md) | **Default off.** Workspace-unified placeholder version; consumers pin by git ref. Manual semver **only** if it's a published contract (an SDK another repo version-pins). |
| [Identifiers](../identifiers.md) | Use `libproduct`'s `product_name!("dev.thmsn.<name>")` for runnable/service variants (e.g. a proxy binary). A pure library may omit it. |
| [Build info](../build-info.md) | Optional, feature-gated (`load_build_info!(optional)`). Off by default. |
| [Testing](../testing.md) | Test the **logic/parsing surface** with inline `#[cfg(test)]` (no separate `tests/` dir). No hard coverage gate — the 80% rule is for services. `scripts/cov.sh` (tarpaulin) optional. |
| [CI/CD](../ci-cd.md) | **Only if it ships an artifact.** A library with a service variant (e.g. libsomeproduct's proxy) gets a Docker/GHCR workflow; a pure crate needs none. |
| [Workflow](../workflow.md) | Commit to `main`. |
| [Error handling](../error-handling.md) | The `liberror`+`thiserror`+`valuable` trinity; serializable, FQN-named errors. |
| [Rust conventions](../rust-conventions.md) | Edition 2024, clippy pedantic + allowlist, git-dep distribution. |
| [`lib*` ecosystem](../lib-ecosystem.md) | This **is** a `lib*` — follow the catalogue conventions; reuse siblings. |
| [Observability](../observability.md) | Optional: `tracing` + `valuable` fields if it logs; no OTLP wiring of its own. |
| Service architecture / Data / Contracts / Auth / Security / Web | **N/A.** |

## Conventions

- **Distribution is git, not crates.io.** Consumers add
  `lib<name> = { git = "https://github.com/charliethomson/lib<name>" }` and pin by ref, or
  use it as a workspace member. Nothing is published to crates.io.
- **Rust 2024 edition**, **clippy pedantic** with a small allowlist, workspace lints.
- **Feature-gate optional surface** (platform backends, `buildinfo`, WAF bypass, raw
  logging) so consumers opt in to weight.
- **`examples/` is the contract.** Each example is a realistic, compilable usage; it
  doubles as the integration test and the API documentation. The README walks through them.
- **Depend on the `lib*` ecosystem** for cross-cutting concerns (`liberror` for
  serializable errors, `libsignal` for graceful shutdown, `liblog`, `libconfig`, …) rather
  than re-solving them.
- If the crate needs `tracing`'s `valuable` support, ship a `.cargo/config.toml` and tell
  consumers to copy it (libsomeproduct pattern).

## Checklist

- [ ] One workspace-unified placeholder version; no per-commit bumps, no tags (unless a
      published contract → then manual semver).
- [ ] Public API is trait-driven and documented in the README.
- [ ] `examples/` cover the real usage paths and compile.
- [ ] Logic/parsing tested inline; optional `scripts/cov.sh`.
- [ ] Optional features gate any heavy or platform-specific surface.
- [ ] CI only if an artifact ships; otherwise none.
- [ ] Root `AGENTS.md` stub points at `standards/`.
