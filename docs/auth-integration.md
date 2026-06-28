# Auth integration

## Rule

Authentication and authorization are centralized in the **auth service**. A product registers
itself and its grants on boot via the **auth SDK** over a TCP/protobuf connection, validates
every request's JWT against the service (not just the token's claims), and gates routes by
**grant**. Identifiers follow `dev.thmsn.<product>` ([identifiers.md](identifiers.md)).

## Registering the app + grants

Declare the app and its grants with `#[derive(App)]` from `auth_sdk`; register idempotently on
startup:

```rust
#[derive(App, Clone, Copy, Debug)]
#[app(id = "dev.thmsn.someproduct", display_name = "SomeProduct",
      description = "A self-hosted SomeProduct service.")]
pub enum Grants {
    #[grant(display_name = "Use",   description = "Access SomeProduct")]
    Use,
    #[grant(display_name = "Admin", description = "Manage server settings")]
    Admin,
}
```

The macro derives grant ids (`dev.thmsn.someproduct.use`, `…​.admin`) and the app descriptor. On
boot, `client.connect_app::<Grants>(&admin_key)` handshakes and upserts the app + grants. Use
the conventional grant set: `use` (or `read`/`write`) and `admin`.

## The handshake (TCP/protobuf)

The SDK opens a persistent, length-delimited protobuf connection and authenticates with an
HMAC-derived key — **no shared password on the wire**:

```
app_key = hex(HMAC-SHA256(admin_key, app_id))
```

The server derives the same key and compares in **constant time** (`subtle::ConstantTimeEq`).
On success the session is `Authenticated { app_id }` and may issue `Register`, `Login`, `Me`,
`Health`, `Info`. `admin_key` is the central TCP HMAC root — a shared Komodo secret
([deployment.md](deployment.md)); reuse it, never regenerate.

## Validating requests

Front the SDK with an `AuthProvider` trait + a gateway that caches `token → User` for a short
TTL:

```rust
#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn login(&self, username: &str, password: &str) -> Result<LoginSuccess, AuthError>;
    async fn me(&self, token: &str) -> Result<User, AuthError>;
}
```

**`me()` is the live authority.** It re-checks the auth DB so disabled users and revoked grants
are rejected immediately — the JWT's embedded claims are never trusted on their own. The short
TTL cache (≈30s–5m) stops every concurrent request serializing through the one TCP client.

## Gating routes

- Extract the JWT from `Authorization: Bearer …`, or from `?token=` **only** on routes that
  can't set headers (asset `<img>` and the WebSocket upgrade).
- Each protected route requires a grant: `Use`/`read` for normal access, `Admin` for
  privileged mutations. **Fail closed:** missing/invalid token → 401; valid token without the
  grant → 403.
- Check grants declaratively (`user.has_grant(Grants::Admin)`), never with hardcoded strings.
- The request logger must **strip query strings** so `?token=` never lands in logs
  ([observability.md](observability.md)).

## Checklist

- [ ] App + grants declared with `#[derive(App)]`, id `dev.thmsn.<product>`.
- [ ] `connect_app` registers idempotently on boot against the auth service.
- [ ] Handshake uses `HMAC-SHA256(admin_key, app_id)`, constant-time compared.
- [ ] An `AuthProvider`/gateway validates via `me()` (live revocation), short-TTL cached.
- [ ] Routes grant-gated, fail-closed (401 vs 403); `?token=` only for assets/WS.
- [ ] Query strings stripped from logs.
