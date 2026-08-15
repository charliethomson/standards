# Auth integration

## Rule

Authentication and authorization are centralized in the **auth service**. A product registers
itself and its grants on boot via the **auth SDK** over a TCP/protobuf connection, validates
every request's JWT against the service (not just the token's claims), and gates routes by
**grant**. Identifiers follow `dev.thmsn.<product>` ([identifiers.md](identifiers.md)).

## Auth modes

A service **declares** its auth mode explicitly — it is never inferred from whether a
secret happens to be set:

- **`sdk`** (default) — federated auth via the auth SDK, as described in this doc. A
  missing or blank `AUTH_ADMIN_KEY` **must fail the service closed at boot** — never
  silently degrade to open. This is the entire point of an explicit mode: a
  misconfigured secret refuses to serve rather than serving everything.
- **`none`** — no auth, for local dev or a service deliberately fronted by an upstream
  iDP. Must be a **loud, logged opt-in**, and **refused on a non-loopback bind** unless
  separately acknowledged. It is never the fallback for a missing key.
- **`proxied`** — auth is enforced by a trusted upstream (gateway forward-auth, an
  OAuth2 proxy, tailnet identity). The service trusts one specific upstream (a signed
  header or network path) and **must reject direct, non-upstream requests** (bind
  loopback or allowlist the gateway).

Cardinal rule: **fail closed on misconfiguration.** `sdk` with no key → refuse to boot;
`none`/`proxied` → explicit and access-restricted. A blank secret must never yield an
open API. The same discipline applies to any service-to-service ingest secret: mandatory
and fail-closed, never "open when unset".

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

## Transport (TLS)

The SDK connection carries the admin key, proxied user passwords, and every JWT/refresh
token — **encrypt it.** Use `connect_tls(addr, domain)` (public-CA verified) rather than
plain `connect`, gated by config so it rolls out per environment:

- `AUTH_TCP_TLS` (bool, default `false` during migration), `AUTH_TCP_ADDR` = the cert's
  `domain:port`, `AUTH_TCP_DOMAIN` = the SNI/verification name.
- **These three are the consuming service's own config fields, so their env vars carry that
  service's prefix** — `SOMEPRODUCT_AUTH_TCP_ADDR`, not bare `AUTH_TCP_ADDR`. Only
  `AUTH_ADMIN_KEY` and `AUTH_MODE` stay unprefixed (the SDK reads those from the environment
  itself). Getting this wrong is silent — the bare name is ignored, the default applies, and the
  service tries plaintext against a TLS-only listener ([configuration.md](configuration.md)).
- Never point `connect_tls` at a bare IP — hostname verification needs the cert's name.
- Plaintext `connect` is acceptable **only** for loopback local dev.

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
  can't set headers (asset `<img>` and the WebSocket upgrade). Prefer a short-lived,
  subject-bound **pre-signed URL** for those header-less routes over a raw `?token=` JWT —
  it leaks one resource, not a full bearer token.
- Each protected route requires a grant: `Use`/`read` for normal access, `Admin` for
  privileged mutations. **Fail closed:** missing/invalid token → 401; valid token without the
  grant → 403.
- Check grants declaratively (`user.has_grant(Grants::Admin)`), never with hardcoded strings.
- The request logger must **strip query strings** so `?token=` never lands in logs
  ([observability.md](observability.md)).

## Refresh tokens

`login`/`refresh` return a rotating refresh token alongside the short-lived JWT. Servers
relay it in an httpOnly cookie (web) or the response body (native clients), never log it,
and never place it in a URL. Client storage and rotation follow
[refresh-tokens.md](refresh-tokens.md): platform-appropriate secret storage, single-flight
rotation, and treating refresh as non-idempotent. If you mint a refresh token, use it —
don't store one you never rotate.

## Checklist

- [ ] Auth mode declared explicitly (`sdk`/`none`/`proxied`); **fails closed** on a
      missing/blank key — no silent open fallback.
- [ ] App + grants declared with `#[derive(App)]`, id `dev.thmsn.<product>`.
- [ ] `connect_app` registers idempotently on boot against the auth service.
- [ ] Handshake uses `HMAC-SHA256(admin_key, app_id)`, constant-time compared.
- [ ] SDK connection uses `connect_tls` (config-gated); plaintext only for loopback dev.
- [ ] An `AuthProvider`/gateway validates via `me()` (live revocation), short-TTL cached.
- [ ] Routes grant-gated, fail-closed (401 vs 403); `?token=` only for assets/WS (prefer
      pre-signed URLs there).
- [ ] Refresh tokens stored/rotated per [refresh-tokens.md](refresh-tokens.md); never logged
      or URL-borne.
- [ ] Query strings stripped from logs.
