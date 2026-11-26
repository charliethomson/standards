# Service architecture

## Rule

A Rust service is a **Cargo workspace layered in one direction**:

```
core  →  db  →  {domain adapters}  →  engine  →  api
```

Dependencies point **inward only** — the arrows never point back out. `core` is pure; `api`
is the binary that composes everything. HTTP is **poem + poem-openapi**.

## The layers

| Crate | Responsibility | Depends on |
|---|---|---|
| **`core`** | Pure domain: canonical models, typed [`Id<T>`](data-persistence.md), the shared `Error`. **Zero I/O.** | — |
| **`db`** | Persistence: the connection pool, migrations, repositories that read/write `core` models. Owns asset storage. | `core` |
| **domain adapters** | Integration clients for one external thing (`payments`, `github`, `ssh`). | `core` |
| **`engine`** | Behaviour/orchestration: turns stored state into actions. Where the logic lives. | `core` (+ `db`/adapters as needed) |
| **`api`** | The binary: HTTP/WebSocket server, composes all layers at runtime, serves requests. | everything |

`core` having zero I/O is the load-bearing rule — it keeps the domain testable and forces the
dependency graph acyclic. Name crates `<product>-<layer>` (`someproduct-core`, `someproduct-db`).

## HTTP: poem-openapi

- **One `…Api` struct per resource group** (`OrdersApi`, `ItemsApi`), methods decorated
  `#[oai(path="…", method="…", operation_id="…")]`. No manual routing.
- Group routes with a `#[derive(poem_openapi::Tags)]` enum.
- **Dependency injection** via the `Data<&AppState>` extractor. `AppState` holds the `Db`,
  `Arc<Config>`, `Arc<Crypto>`, the token verifier ([auth-integration.md](auth-integration.md)), the `WsHub`, and any caches/queues.
- Request/response DTOs derive `poem_openapi::Object`; response DTOs that also go over the
  WebSocket additionally derive `Serialize`.
- Domain errors convert at the boundary with `.map_err(to_poem)` — see
  [error-handling.md](error-handling.md).

### Standard mount map

```
/api/v1/*        OpenAPI service (bearer access token; group/scope-gated)
/api/auth/config issuer, audience, scopes, client ids (public, outside OpenAPI)
/api/auth/*      web sign-in: login, callback, refresh, logout (cookie, outside OpenAPI)
/openapi.json    emitted contract (public)
/docs            Scalar interactive docs (public)
/ws              WebSocket (access token via header or ?token=)
/health          unauthenticated liveness probe
```

There is no `/api/login`: people sign in at the identity provider. `/api/auth/*` exists only
when the product has a web client ([auth-integration.md](auth-integration.md#web-the-server-side-exchange)).
A repo still on the legacy auth SDK keeps its `/api/login` until it migrates.

## Composition & shutdown

The `api` binary: load config → init telemetry ([observability.md](observability.md)) →
connect `Db` (runs migrations) → build `AppState` → build the poem app → serve with
**graceful shutdown**:

```rust
poem::Server::new(TcpListener::bind(&bind))
    .run_with_graceful_shutdown(app, shutdown_signal(), Some(Duration::from_secs(10)));
```

`shutdown_signal()` resolves on Ctrl-C **or** (Unix) SIGTERM, giving a 10-second drain so
`docker stop` finishes in-flight requests instead of hard-killing. Use [`libsignal`](lib-ecosystem.md)
for the cross-platform signal wait.

## Concurrency

- `#[tokio::main]`, `tokio` with `features = ["full"]`.
- **Per-resource serial work** uses a queue keyed by resource id: jobs for one resource run
  FIFO/serially, distinct resources run in parallel, each job is its own spawned task so a
  panic is isolated. (The per-agent `JobQueue` is the canonical pattern.)
- Long-lived stateful components follow an **actor loop**: an owned `tokio::task` consuming a
  command channel (`mpsc`), publishing events on a `broadcast` channel.

## Checklist

- [ ] Workspace members are `core`, `db`, (`adapters`), `engine`, `api`; deps point inward only.
- [ ] `core` performs no I/O.
- [ ] HTTP via poem-openapi; one `…Api` struct per resource; DI via `Data<&AppState>`.
- [ ] Standard mount map (`/api/v1`, `/api/auth/config`, `/api/auth/*`, `/openapi.json`, `/docs`,
      `/ws`, `/health`); no `/api/login` (sign-in is the IdP's, see
      [auth-integration.md](auth-integration.md)).
- [ ] Graceful shutdown with a 10s drain on Ctrl-C/SIGTERM.
- [ ] Per-resource serial work uses a keyed queue; stateful components are actor loops.
