# Testing

## Rule

**Aim for >80% coverage on services. Do not unit-test UIs.** Test the surfaces where bugs
are costly and regressions silent — server logic, protocol/state handling, parsing — and
spend nothing on brittle UI unit tests. Verify UI behaviour with a small number of
**hermetic end-to-end** flows instead.

This matters more because much of the code is AI-generated: a coverage floor on the service
is the safety net that catches a regression a quick read would miss.

## Server (Rust) — the 80% floor

- Tests are inline `#[cfg(test)]` / `#[tokio::test]`; run with `cargo test --workspace`.
- **`cargo tarpaulin` enforces `fail-under = 80`** via `server/tarpaulin.toml`
  ([template](../templates/rust/tarpaulin.toml)), gated in CI ([ci-cd.md](ci-cd.md)).
- Exclude **wiring, not logic**: bootstrap `main.rs` (config load, signal handling, server
  spawn — exercised by integration tests), and genuine I/O edges like an external-service
  gateway, each with a justifying comment. Everything else counts toward the floor.

## Web — unit + hermetic e2e

- **Vitest** for unit/component tests (`jsdom`); `npm run test`.
- **Playwright** for e2e — run against a **mocked server**, not a live one: fake REST via
  route interception + an injected fake WebSocket. CI needs no backend, so the suite is
  hermetic and fast. Keep it to the critical flows, run serially in CI (1 worker).
- Lint with **max-warnings 0**; the **codegen drift check** (`codegen:check`) is part of the
  test gate — the typed client must regenerate cleanly from the committed OpenAPI spec.

## Native (Apple / Windows) — test the core, not the views

- Test the **shared, testable core** and skip the UI:
  - Apple: `swift test` on `<Product>Kit` — protocol, dedupe, reconnect, view models.
  - Windows: xUnit on `<Product>Core` — the same high-risk surfaces; target `net10.0` so it
    also runs on Linux CI.
- Each client also runs a **codegen drift check** (`git diff --exit-code` after regenerating
  its API client) — a generated client drifting from the spec is a test failure.
- **Do not** write unit tests for SwiftUI/WinUI views.

## Libraries — logic surface, no gate

Libraries are held to a **softer** bar than services. Test the **logic/parsing surface**
with inline `#[cfg(test)]` (no separate `tests/` dir); `examples/` double as integration
coverage. No hard coverage gate — the 80% floor is for services. An optional
`scripts/cov.sh` (tarpaulin, HTML/Lcov) is fine for spot-checks. See
[archetypes/library.md](archetypes/library.md).

## Why these lines

- **Services get the gate** because their logic is invisible at runtime until it's wrong,
  and they're the shared behaviour every client depends on.
- **UIs don't get unit tests** because they're high-churn and the tests are brittle and
  low-signal; a few hermetic e2e flows catch the regressions that matter for far less upkeep.
- **The core, not the views**, on native — the protocol/dedupe/reconnect/VM layer is where
  correctness lives and it's cheaply testable; the views are thin.

## Checklist

- [ ] Server has `tarpaulin.toml` with `fail-under = 80`, enforced in CI.
- [ ] Coverage excludes only wiring/I/O edges, each justified.
- [ ] Web: Vitest unit + Playwright e2e against a mocked server; lint max-warnings 0.
- [ ] Native: core unit tests (protocol/dedupe/reconnect/VMs); no view unit tests.
- [ ] Every generated client has a drift check in CI.
- [ ] Libraries test the logic surface inline; no coverage gate.
