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

## No change-detector tests

**A test that fails when the implementation changes, rather than when the behaviour changes,
is deleted or rewritten. It is never "fixed" to match the new code.**

A change-detector test is one whose assertions are a transformation of the code under test:
it mocks every collaborator and verifies that each was called, in order, with the arguments
the implementation happens to pass. Such a test is a checksum of the implementation. A
correct program and a broken one are equally likely to pass it, and every refactor breaks it
and has to be mechanically re-derived from the new code. That is negative value: no defects
caught, and a maintenance tax on every change. (Alex Eagle, *Change-Detector Tests Considered
Harmful*, Google Testing on the Toilet, 2015.)

This matters more here than at Google. Most of the fleet's code is agent-written, and an
agent chasing the 80% floor above will reach for exactly this shape, because it is the
cheapest way to execute lines. **Coverage counts lines executed, not behaviour verified.** A
change detector that lifts coverage is still deleted; if the floor then fails, the answer is
a behaviour test or a justified wiring exclusion, never a mock-verify test.

### How to recognise one

Any of these is a strong signal:

- The only assertions are that collaborators were called (`expect_x().times(1)` on a mock,
  `toHaveBeenCalledWith`, `verify(...)`), especially in a prescribed order.
- The test could not have been written from the function's contract alone; you had to read
  the implementation to know what to assert.
- A pure refactor (rename, extract, reorder, change a parameter) broke it and the fix was
  mechanical. If you are applying the same edit to many tests after a refactor, they are all
  change detectors.
- A snapshot was committed without anyone deciding the captured output was *right*.

The two questions that settle it: **would this fail if the code were wrong?** and **would
this still pass if the code were correctly refactored?** A test worth keeping answers yes to
both.

### The shape to avoid, in fleet idioms

```rust
// engine: a thin orchestration
pub async fn ingest(&self, item: NewItem) -> Result<Item> {
    let item = self.normaliser.normalise(item)?;
    self.store.insert(item).await
}

// ❌ change detector: restates the body as expectations, verifies nothing about the result
#[tokio::test]
async fn ingest_normalises_then_inserts() {
    let mut normaliser = MockNormaliser::new();
    let mut store = MockStore::new();
    normaliser.expect_normalise().times(1).returning(Ok);
    store.expect_insert().times(1).returning(|i| Ok(i));
    Engine::new(normaliser, store).ingest(new_item()).await.unwrap();
}

// ✅ behaviour: real in-memory store, assert on what came out
#[tokio::test]
async fn ingest_stores_a_normalised_item() {
    let engine = Engine::in_memory().await;
    let stored = engine.ingest(new_item_titled("  Hello ")).await.unwrap();
    assert_eq!(stored.title, "Hello");
    assert_eq!(engine.store.get(stored.id).await.unwrap().title, "Hello");
}
```

```ts
// ❌ change detector: asserts the hook's wiring, not what the user sees
it("uses the orders query", () => {
  renderHook(() => useOrders());
  expect(useQuery).toHaveBeenCalledWith(expect.objectContaining({ queryKey: ["orders"] }));
});

// ✅ behaviour: mocked route, assert on the rendered outcome
it("lists the orders the server returns", async () => {
  server.get("/api/orders", () => [order({ id: "ord_1", title: "Widgets" })]);
  render(<Orders />);
  expect(await screen.findByText("Widgets")).toBeInTheDocument();
});
```

### What to do instead

- **Assert on outputs and state**, not on calls: the return value, the row in the store, the
  rendered text, the frame on the wire, the error variant. This is why the fleet's data layer
  is sqlx + SQLite ([data-persistence.md](data-persistence.md)): an in-memory database is
  cheap, so engine tests can use the real store instead of mocking it.
- **Mock only at genuine I/O edges** you cannot run hermetically: an external-service
  gateway, the clock, the network. A mock there is a *stub that returns data*, not a
  *spy that asserts calls*. Even then, prefer asserting the effect on your side (what got
  persisted, what got emitted) over the shape of the outbound call.
- **Pure orchestration has nothing to unit-test.** If a function only sequences
  collaborators, either test it through an integration path that exercises the real
  collaborators, or exclude it from coverage as wiring with a justifying comment (the
  "wiring, not logic" rule above). Do not manufacture a mock-verify test for it.
- **When a refactor breaks a test and the fix would be mechanical, stop.** Rewrite it
  against behaviour or delete it, in the same commit as the refactor. Never land the
  mechanical fix.
- **Snapshots need a reviewer.** A committed snapshot is an assertion someone chose. Keep
  them small, name what they pin, and read them when they change.

### Change detection that is the point

Some checks in this fleet detect change on purpose, and they are fine, because the *change
itself* is the defect they guard against:

- the **codegen drift check** ([contracts.md](contracts.md)): a generated client diverging
  from the committed spec is exactly the failure;
- a golden file for a wire format or on-disk layout you have promised to keep stable;
- a migration checksum.

The distinction is whether the thing pinned is a **contract** (drift is a bug) or an
**implementation** (drift is a refactor). Pin contracts. Never pin implementations.

## Why these lines

- **Services get the gate** because their logic is invisible at runtime until it's wrong,
  and they're the shared behaviour every client depends on.
- **UIs don't get unit tests** because they're high-churn and the tests are brittle and
  low-signal; a few hermetic e2e flows catch the regressions that matter for far less upkeep.
- **The core, not the views**, on native — the protocol/dedupe/reconnect/VM layer is where
  correctness lives and it's cheaply testable; the views are thin.
- **Change detectors are banned** because a coverage floor is only a safety net if the tests
  under it can fail for the right reason. A mock-verify test cannot, and it makes every refactor
  cost more, which for agent-written code is the wrong trade twice.

## Checklist

- [ ] Server has `tarpaulin.toml` with `fail-under = 80`, enforced in CI.
- [ ] Coverage excludes only wiring/I/O edges, each justified.
- [ ] Web: Vitest unit + Playwright e2e against a mocked server; lint max-warnings 0.
- [ ] Native: core unit tests (protocol/dedupe/reconnect/VMs); no view unit tests.
- [ ] Every generated client has a drift check in CI.
- [ ] Libraries test the logic surface inline; no coverage gate.
- [ ] No test asserts only that collaborators were called; assertions are on outputs and state.
- [ ] Mocks stub I/O edges only; pure orchestration is integration-tested or excluded as wiring.
- [ ] A refactor that breaks tests gets those tests rewritten or deleted, not mechanically patched.
