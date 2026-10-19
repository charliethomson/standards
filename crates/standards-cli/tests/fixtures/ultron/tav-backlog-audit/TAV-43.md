# TAV-43: API contract: 201 on create, uniform pagination, role enum (breaking)
state=backlog priority=medium created=2026-09-17T10:56:25.680419668+00:00

**Split out of TAV-28**, whose additive half landed in `8d8a8af`. What remains are the parts that **change runtime behaviour for all three generated clients**, so they need a deliberate change with a migration rather than a slot in a review sweep.

### 1. Creates return `200`, not `201`

Every create operation answers `200`. Switching to `201` is correct REST, but it is a **breaking change for every client**:

* Swift: `swift-openapi-generator` models each documented status as a separate case, so hand-written code doing `.ok.body` stops compiling and has to become `.created.body`. `TavernService.swift` has ~15 such call sites.
* C#: Kiota is more forgiving, but any explicit status handling changes.
* TypeScript: `openapi-fetch` returns `data` for any 2xx, so this one is largely transparent.

Needs: change the handlers, regenerate, update the Swift service layer in the same commit, ship server and clients together (an old client against a new server would treat `201` as unhandled).

### 2. Uniform pagination

Framed as a cleanup, but it is really an **API expansion**. Only `list_messages` paginates today (`limit`/`offset` query params); every other list returns the whole collection. "Uniform" therefore means *adding* pagination to characters, chats, personas, presets, lorebooks, members, images, settings — new query params, a decision about whether to wrap responses in an envelope (`{items, total}`) or keep bare arrays with headers, and client work to drive it.

Worth doing before any collection gets large, but it is a design task with its own trade-offs, not a contract tidy-up.

### 3. `role` as an enum

`MessageRequest.role` is a freeform `String` in the DTO while the domain (`core::model::MessageRole`) is closed: `user` / `assistant` / `system`. Modelling it as an enum in the spec makes generators emit a real enum type — which is the *point*, and also the breaking part for hand-written client code that passes strings.

Lower risk than the other two and could go first.

### Acceptance criteria (carried over)

- [ ] `role` is an enum.
- [ ] Uniform pagination; creates return `201`.
- [ ] Clients regenerated; drift check green.

Note the drift gate now exists (TAV-3, `committed_openapi_json_matches_the_server`), so any of these will fail CI until the spec and all three clients are regenerated together — which is the desired behaviour.

---
_Imported from [THM-307](https://linear.app/thmsn/issue/THM-307/api-contract-201-on-create-uniform-pagination-role-enum-breaking)._

