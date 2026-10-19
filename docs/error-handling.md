# Error handling

## Rule

Two complementary layers:

1. **Service errors** — a categorised domain `Error` enum (`thiserror`) in `core`, mapped to
   HTTP status at the API boundary by a `to_poem()` function. The domain layer never knows
   about HTTP.
2. **Library / cross-boundary errors** — the **`liberror` + `thiserror` + `valuable` trinity**:
   serializable error enums that preserve the error chain and carry structured fields into
   logs.

## Service domain errors

Define one categorised enum in `core`; variants map cleanly onto status codes:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{entity} not found")]      NotFound { entity: &'static str },
    #[error("invalid input: {0}")]      Invalid(String),
    #[error("conflict: {0}")]           Conflict(String),
    #[error("unauthorized")]            Unauthorized,
    #[error("storage error: {0}")]      Storage(String),
    #[error("upstream error: {0}")]     Upstream(String),
    #[error(transparent)]               Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
pub type Result<T> = std::result::Result<T, Error>;
```

Map to HTTP **in the `api` crate only**. The mapper picks the status, records the failure
on the request span, and returns a JSON body with **fixed client text**: a storage or
upstream error's `Display` carries SQL, paths and upstream bodies, which belong in the log
and never in a response.

```rust
pub fn to_poem(err: &Error) -> poem::Error {
    let status = match err {
        Error::NotFound { .. } => StatusCode::NOT_FOUND,
        Error::Invalid(_)      => StatusCode::BAD_REQUEST,
        Error::Conflict(_)     => StatusCode::CONFLICT,
        Error::Unauthorized    => StatusCode::UNAUTHORIZED,
        Error::Upstream(_)     => StatusCode::BAD_GATEWAY,
        Error::Storage(_) | Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let message = match err {
        Error::Upstream(_) => "an upstream service failed".to_owned(),
        Error::Storage(_) | Error::Other(_) => "internal error".to_owned(),
        client => client.to_string(), // written for the caller
    };
    // Onto the request span, through the request layer's task-local handle: the
    // layer's one `http completed` line carries it (ERROR for a 5xx). No log
    // here, or a 5xx logs twice.
    request::record_error(err.key(), status.is_server_error().then(|| err.to_string()));
    poem::Error::from_response(request::error_response(status, err.key(), &message))
}
```

`Invalid` and `Conflict` messages are written for the client, so they pass through; a
variant whose text isn't, doesn't. The detail recorded for a 5xx is still sanitised: no
bodies, no URLs with tokens ([tracing.md](tracing.md#privacy)).

Handlers: `state.db.list_x().await.map_err(|e| to_poem(&e))?`. Templates:
[`templates/rust/error.rs`](../templates/rust/error.rs) and the request layer it records into,
[`templates/rust/request_layer.rs`](../templates/rust/request_layer.rs).

**Error bodies are JSON:** `{"message": "...", "$type": "<error key or kind>", "traceId":
"<32 hex>"}`, the same id the `x-<product>-trace-id` header carries. Clients show `message`
and `Ref: <traceId[..8]>` ([tracing.md](tracing.md#client--server-contract)).

## Serializable errors: the trinity

Errors that cross a serialization boundary — returned from a `lib*` crate, logged structurally,
or surfaced in an API body — are **serializable, chain-preserving, and keyed**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, valuable::Valuable)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase",
        tag = "$type", content = "context")]
pub enum ConfigError {
    #[error("Failed to read config at \"{path}\": {inner_error}")]
    #[serde(rename = "dev.thmsn.someproduct.server.config.error.read")]
    Read { path: String, inner_error: AnyError },

    #[error("Config file was modified externally since it was last loaded")]
    #[serde(rename = "dev.thmsn.someproduct.server.config.error.stale")]
    Stale,
}
pub type ConfigResult<T> = Result<T, ConfigError>;
```

Three derives, always together:
- **`thiserror::Error`** — the human `Display` message.
- **`serde` with `tag = "$type", content = "context"`** — the machine-readable envelope. Every
  variant carries a stable **error key** (`$type`) via `#[serde(rename = …)]` — see below.
- **`valuable::Valuable`** — lets `tracing` log the error as **structured fields** (requires the
  `tracing_unstable` cfg, see [observability.md](observability.md)).

A serialized error is then
`{ "$type": "dev.thmsn.someproduct.server.config.error.read", "context": { "path": "…",
"innerError": { … } } }` — the human message and the machine key are decoupled.

**Wrap foreign errors** with [`liberror`](lib-ecosystem.md)'s `AnyError`, which captures any
`std::error::Error` as `{ $type, context: { message, inner_error } }`, preserving the source
chain through serialization:

```rust
SomeVariant { inner_error: AnyError },  // from sqlx::Error, io::Error, oauth2::Error, …
```

## Error keys

Every serializable error variant gets a stable, **reverse-DNS error key** — the `$type`
discriminant — so a serialized error names *exactly what failed*, independent of its wording.
The grammar mirrors the reverse-domain identity convention ([identifiers.md](identifiers.md)):

```
dev.thmsn.<root>[.<area…>].error.<kind>
```

- **`<root>`** — the product (`<product>`), or for a `lib*` crate its name **with the `lib`
  prefix dropped** (`libsomeproduct` → `someproduct`; in the fleet, `libbuildinfo` → `build_info`).
- **`<area…>`** — one or more segments locating the failure: for a product, the **component**
  then the module/subject (`server.config`); for a library, the module path (`extract.git`).
  Be specific — the whole point is that the key points straight at the code path.
- **`error`** — a literal segment, always second-to-last.
- **`<kind>`** — the variant, `snake_case`, naming the specific failure.

| Key | Means |
|---|---|
| `dev.thmsn.someproduct.server.config.error.read` | the server's config module failed to read |
| `dev.thmsn.someproduct.worker.error.spawn` | the worker failed to spawn |
| `dev.thmsn.someproduct.extract.git.error.discover` | (library) the git-extract step couldn't discover the repo |

**Why so specific:** when an error shows up in a log line or an API body, its key is a search
string that takes you straight to the one place it's raised — that's the debugging payoff. Keep
keys narrow; rename the `Display` message freely, but treat the **key as part of the contract**.

## Two layers, one flow

`liberror`-wrapped library errors bubble up into a service's domain `Error` (often as
`Storage`/`Other`), which `to_poem` then turns into a status code. The library layer stays
serializable and loggable; the service layer stays HTTP-mappable; neither leaks the other's
concerns.

## Checklist

- [ ] `core` defines a categorised `Error` enum + a `Result<T>` alias.
- [ ] `to_poem` (HTTP mapping) lives in `api` only; domain code is HTTP-agnostic.
- [ ] Error bodies are JSON `{message, $type, traceId}` with fixed client text for 5xx;
      the mapper records `error.type` on the request span and does not log.
- [ ] Serializable errors derive `thiserror` + `serde` (`tag="$type"`, `content="context"`,
      `rename_all`/`rename_all_fields = "camelCase"`) + `valuable`.
- [ ] **Every** serializable variant has a reverse-DNS error key via `#[serde(rename = …)]`:
      `dev.thmsn.<root>.<area>.error.<kind>` — specific to the code path, `snake_case` leaf.
- [ ] Library keys drop the `lib` prefix from the root (`libsomeproduct` → `someproduct`).
- [ ] Keys are treated as a stable contract (rename messages freely, not keys).
- [ ] Foreign errors wrapped in `liberror::AnyError` to preserve the chain.
