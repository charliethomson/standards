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

Map to HTTP **in the `api` crate only**:

```rust
pub fn to_poem(err: Error) -> poem::Error {
    let status = match &err {
        Error::NotFound { .. } => StatusCode::NOT_FOUND,
        Error::Invalid(_)      => StatusCode::BAD_REQUEST,
        Error::Conflict(_)     => StatusCode::CONFLICT,
        Error::Unauthorized    => StatusCode::UNAUTHORIZED,
        Error::Upstream(_)     => StatusCode::BAD_GATEWAY,
        Error::Storage(_) | Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    poem::Error::from_string(err.to_string(), status)
}
```

Handlers: `state.db.list_x().await.map_err(to_poem)?`. Template:
[`templates/rust/error.rs`](../templates/rust/error.rs).

## Library errors: the trinity

Errors that cross a serialization boundary (returned from a `lib*` crate, logged structurally,
or surfaced in an API body) are **serializable and chain-preserving**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, valuable::Valuable)]
#[serde(tag = "$type", content = "context", rename_all = "camelCase")]
pub enum ConfigError {
    #[serde(rename = "dev.thmsn.config.error.read")]
    #[error("Failed to read config at \"{path}\": {inner_error}")]
    Read { path: String, inner_error: AnyError },
    #[error("Config file was modified externally since it was last loaded")]
    Stale,
}
pub type ConfigResult<T> = Result<T, ConfigError>;
```

Three derives, always together:
- **`thiserror::Error`** — the `Display` message.
- **`serde` with `tag = "$type", content = "context"`** — a machine-readable envelope. Name
  variants with an **FQN** (`dev.thmsn.<lib>.error.<kind>`) via `#[serde(rename = …)]`.
- **`valuable::Valuable`** — lets `tracing` log the error as **structured fields**, not a
  flattened string (requires the `tracing_unstable` cfg, see [observability.md](observability.md)).

**Wrap foreign errors** with [`liberror`](lib-ecosystem.md)'s `AnyError`, which captures any
`std::error::Error` as `{ $type, context: { message, inner_error } }`, preserving the source
chain through serialization:

```rust
SomeVariant { inner_error: AnyError },  // from sqlx::Error, io::Error, oauth2::Error, …
```

## Two layers, one flow

`liberror`-wrapped library errors bubble up into a service's domain `Error` (often as
`Storage`/`Other`), which `to_poem` then turns into a status code. The library layer stays
serializable and loggable; the service layer stays HTTP-mappable; neither leaks the other's
concerns.

## Checklist

- [ ] `core` defines a categorised `Error` enum + a `Result<T>` alias.
- [ ] `to_poem` (HTTP mapping) lives in `api` only; domain code is HTTP-agnostic.
- [ ] Library/cross-boundary errors derive `thiserror` + `serde($type/context)` + `valuable`.
- [ ] Serializable error variants carry FQN names (`dev.thmsn.<lib>.error.<kind>`).
- [ ] Foreign errors wrapped in `liberror::AnyError` to preserve the chain.
