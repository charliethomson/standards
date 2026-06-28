# API contracts

## Rule

The server **owns the contract**. It emits an OpenAPI document for its REST surface and an
AsyncAPI document for its real-time surface; both are **committed to the repo**, and every
client is **generated from them** with a **CI drift check**. Clients never hand-write types
the server already describes.

This is the "contract = `api/openapi.json`" half of the product's three sources of truth
([archetypes/full-stack-product.md](archetypes/full-stack-product.md)).

## REST: OpenAPI

- poem-openapi derives the spec from the `…Api` structs. Expose it two ways:
  - an `openapi` subcommand that prints the JSON (`<bin> openapi > api/openapi.json`),
  - the live `/openapi.json` route + Scalar docs at `/docs`.
- **Commit `api/openapi.json`.** It's the artifact clients build against.
- **Generate typed clients** from it, per platform:
  - Web: `openapi-typescript` → `schema.d.ts`, consumed by `openapi-fetch`.
  - Apple: `swift-openapi-generator` → `Types.swift` + `Client.swift`.
  - Windows: Kiota → generated client.
- **Drift check in CI** ([ci-cd.md](ci-cd.md)): regenerate and `git diff --exit-code`. A diff
  means the committed client is stale — fail the build. This keeps every client honest against
  the server.

## Real-time: AsyncAPI + the envelope

WebSocket/SSE frames are **server-to-client JSON** with a uniform envelope, documented in
`api/asyncapi.md`:

```jsonc
{ "kind": "<dotted.event.name>", "data": { /* DTO or delta */ } }
// some products also thread a correlation id: { "kind": "...", "order_id": "<uuid|null>", "data": {} }
```

- `kind` is a stable dotted name (`order.created`, `job.started`, `assignment.updated`).
- `data` reuses the same DTOs as the REST API (so generated types apply).
- **Clients filter by `kind` + ids and apply idempotently**; on a gap they refetch via REST.
- **SSE** tags each frame with an `event_type` string mirroring `kind`; keep-alive ping ~30s.

## Transport: broadcast hub

Real-time fan-out is a `tokio::broadcast` channel with a bounded backlog (e.g. 256). The
server `publish()`es serialized events to all subscribers; slow clients that lag past the
backlog are dropped and expected to resync over REST. This `WsHub` lives in `AppState`.

## Inter-service contract: protobuf

Service-to-service RPC over a persistent TCP connection uses **protobuf (prost)** with
length-delimited framing and a `oneof result { ok; error }` envelope. This is the auth SDK's
wire format — see [auth-integration.md](auth-integration.md). REST/JSON is for clients;
protobuf is for the hot internal auth path.

## Checklist

- [ ] Server emits OpenAPI via an `openapi` subcommand **and** `/openapi.json` + `/docs`.
- [ ] `api/openapi.json` is committed.
- [ ] Every client is generated from it (openapi-typescript / swift-openapi-generator / Kiota).
- [ ] CI drift check (`git diff --exit-code`) on every generated client.
- [ ] Real-time frames use the `{kind, data}` envelope, documented in `api/asyncapi.md`.
- [ ] Fan-out via a bounded `broadcast` hub; clients resync over REST on gaps.
