# Auth integration

## Rule

Identity lives in **Authelia** (`https://sso.dev.thmsn.dev`), an OpenID Connect provider.
Products never see a password and never run a login form of their own.

- Every **client** of a product (web, iOS, macOS, Windows, CLI, a calling service) is an
  OIDC client registered with Authelia.
- Every product **server** is an OAuth 2.0 resource server. It validates JWT access tokens
  (RFC 9068) **locally** against the issuer's JWKS, and gates routes on the `groups` claim
  (people) or on scopes (services).
- The product's identifier `dev.thmsn.<product>` ([identifiers.md](identifiers.md)) is also
  its token **audience**.
- Server-side validation and client-side token handling come from the shared
  [`liboidc`](lib-ecosystem.md) crate (`server` and `client` features). Swift and C# clients
  use their platform's APIs instead.

The old central **auth service** and its SDK are legacy. Apps still on it follow
[Legacy: the auth service SDK](#legacy-the-auth-service-sdk) until they migrate. Nothing new
is built on it.

## Which standard a repo is on

A repo on OIDC needs no declaration. A repo still on the legacy SDK says so in its
`AGENTS.md`, on the line after its `Standards:` line:

```
Standards: archetype=full-stack-product product=someproduct
Auth: legacy-sdk
```

That line means "audit this repo's auth against the Legacy section, not the rest of this
doc". It is temporary: it comes out in the same change that moves the app to OIDC. A new repo
never starts on `legacy-sdk`.

## Auth modes

A server **declares** its mode in `<PRODUCT>_AUTH_MODE`. It is never inferred from whether
something happens to be set.

- **`oidc`** (default). Local JWT validation as described here. The server **refuses to
  boot** when `<PRODUCT>_OIDC_ISSUER` or `<PRODUCT>_OIDC_AUDIENCE` is blank, or when the
  issuer's discovery document or JWKS can't be fetched at boot after a bounded retry.
- **`none`**. No auth, for local dev. A **loud, logged opt-in**, and **refused on a
  non-loopback bind** unless separately acknowledged. Never the fallback for missing config.
- **`proxied`**. Auth is enforced by one trusted upstream (gateway forward-auth, tailnet
  identity). The server trusts that upstream only, through a signed header or network path,
  and **rejects direct requests** (bind loopback or allowlist the gateway). Authelia
  forward-auth in the homelab gateway is the canonical upstream for an app with no sign-in
  of its own.

Cardinal rule: **fail closed on misconfiguration.** A blank issuer or audience must never
yield an open API. The same discipline applies to any service credential: mandatory and
fail-closed, never "open when unset".

## Config

All of these are the product's own fields, so they carry its prefix
([configuration.md](configuration.md)):

| Variable | Value | Notes |
|---|---|---|
| `<PRODUCT>_AUTH_MODE` | `oidc` / `none` / `proxied` | default `oidc` |
| `<PRODUCT>_OIDC_ISSUER` | `https://sso.dev.thmsn.dev` | compared **exactly** with the token's `iss` and the discovery document's `issuer` |
| `<PRODUCT>_OIDC_AUDIENCE` | `dev.thmsn.<product>` | the token's `aud` must contain it |
| `<PRODUCT>_OIDC_SERVICE_CLIENTS` | comma list of client ids | services allowed to call service routes; empty when there are none |
| `<PRODUCT>_OIDC_CLIENT_SECRET` | secret | web server-side exchange only; a Komodo secret ([deployment.md](deployment.md)) |

All traffic to the issuer is HTTPS. The issuer resolves on internal DNS and the tailnet only,
so the server's host must be able to reach it.

## Declaring the permission model

A product declares its permissions **once, in code**, as a table from variant to name (a
const table or derive from `liboidc`). Route checks use the table, never string literals.

- **People** get **groups**: `<slug>` grants Use, `<slug>-admin` grants Admin, and
  `<slug>-write` (or another `<slug>-<grant>`) when a product needs it. `<slug>` is the last
  segment of the app id: `dev.thmsn.someproduct` → `someproduct`, `someproduct-admin`.
- **Services** get **scopes**: `<slug>.<grant>`, e.g. `someproduct.ingest`.
- **Admin is checked in addition to Use**, so an admin is in both groups.

There is no registration call at boot. The Authelia config is the registration, and it lands
in the same change as the app's move to OIDC (or its first release):

- one client per client surface (see the next section);
- an `authorization_policies.<slug>` entry, default `deny`, with `group:<slug>` allowed;
- the groups, assigned to people in the user list.

Giving a person access to an app is adding them to its groups.

## Clients

Client ids are `<slug>-<surface>`. Every client lists **only** its own product's audience and
grants it without the client asking (`requested_audience_mode: implicit`), and uses the
issuer's RS256 key for JWT access tokens.

| Surface | Client id | Type | Flow | Redirect |
|---|---|---|---|---|
| Web | `<slug>-web` | confidential | authorization code + PKCE, run by the product server | `https://<host>/api/auth/callback` |
| iOS / macOS | `<slug>-ios`, `<slug>-macos` (or one `<slug>-apple`) | public | authorization code + PKCE in `ASWebAuthenticationSession` | `dev.thmsn.<product>.<platform>:/oauth/callback`, or a universal link where the issuer refuses a custom scheme |
| Windows | `<slug>-windows` | public | authorization code + PKCE in the system browser | `http://127.0.0.1:<any port>/callback` |
| CLI | `<slug>-cli` | public | device authorization grant (RFC 8628); loopback PKCE as fallback | loopback, as Windows |
| Service | `<slug>-<service>` | confidential | `client_credentials` | none |

- Public clients authenticate with nothing but PKCE (`S256`). No client secret ships in an
  app or a CLI.
- People's clients request `openid offline_access groups` (plus `profile` when the app shows a
  username). Asking for `offline_access` shows a consent screen on each interactive sign-in;
  that one click is accepted. Refreshes are silent.
- Access tokens last **10 minutes**, refresh tokens **30 days**.
- One CLI that calls several products may request several audiences, so one sign-in covers all
  of them. That client's entry lists each audience; every server still checks only its own.

## Discovery: `GET /api/auth/config`

Every `oidc` server exposes a public, unauthenticated route, outside the OpenAPI document:

```json
{
  "issuer": "https://sso.dev.thmsn.dev",
  "audience": "dev.thmsn.someproduct",
  "scopes": ["openid", "offline_access", "groups", "profile"],
  "clients": { "ios": "someproduct-ios", "windows": "someproduct-windows", "cli": "someproduct-cli" }
}
```

Native apps and CLIs read it from whatever server URL the user entered, then talk to the
issuer it names. **No client build hardcodes the issuer or a client id.**

## Validating requests

Local verification is the authority. A request is authenticated when its access token passes
**all** of:

- `alg` is exactly **RS256**, pinned in code, never taken from the token;
- header `typ` is **`at+jwt`** (this is what stops an ID token, signed by the same key, from
  passing as an access token);
- `iss` equals `<PRODUCT>_OIDC_ISSUER` exactly;
- `aud` contains `<PRODUCT>_OIDC_AUDIENCE`;
- `exp`, `nbf`, `iat` hold with 60s leeway; `exp` and `sub` are present;
- the signature verifies against a key from the issuer's JWKS.

The verifier then builds a `Principal { sub, client_id, groups, scopes, kind }`. Scopes come
from `scp` (an array); accept a space-delimited `scope` string too.

- **JWKS cache.** Keys by `kid`, refreshed in the background. An unknown `kid` triggers one
  refetch, rate-limited (at most one per 30s) so a forged `kid` can't hammer the issuer. A
  failed refresh keeps the last good set.
- **No per-request call to the issuer.** No introspection on every request and no
  token-to-user cache. The trade is stated in [Revocation](#revocation).

## Gating routes

- Token from `Authorization: Bearer`. See [Header-less routes](#header-less-routes) for the
  exceptions.
- **401** with `WWW-Authenticate: Bearer error="invalid_token"` for a missing, malformed,
  expired, wrong-issuer or wrong-audience token. **403** for a valid token without the group
  or scope.
- A **user route** requires the group (`<slug>`, plus `<slug>-admin` for Admin) **and
  refuses service tokens**.
- A **service route** requires `kind = Service` and the scope.
- A token is a **service token** iff `sub == client_id` **and** that client id is in
  `<PRODUCT>_OIDC_SERVICE_CLIENTS`. Everything else is a user token.
- Checks go through the declared table (`principal.require(Grants::Admin)?`), never literals.
- The request logger **strips query strings** ([observability.md](observability.md)).

Two gates apply, and both are required. Authelia's authorization policy refuses a user
outside `<slug>` at sign-in, before any token exists. The server's per-request group check is
the one that holds after sign-in, and the only place Admin is enforced.

## Long-lived connections

A WebSocket or SSE stream outlives the token that opened it. The server **caps each connection
at the access token's remaining lifetime** (plus jitter), or the client re-authenticates
in-band before it runs out. Either way a revoked user drops within the same window as a
plain request.

## Header-less routes

`<img>` assets and the WebSocket upgrade can't set headers. Those routes alone may take the
access token as `?token=`. Prefer a short-lived pre-signed URL bound to `sub` over a raw
token: it leaks one resource, not a bearer credential.

## Web: the server-side exchange

The web client never talks to the issuer from JavaScript. The product server does the OAuth
work as the confidential client `<slug>-web`:

```
/api/auth/login      302 to the issuer's authorize endpoint (state + PKCE)
/api/auth/callback   exchanges the code, sets the refresh cookie, 302 back into the SPA
/api/auth/refresh    rotates the refresh token, returns a fresh access token
/api/auth/logout     revokes the refresh token at the issuer, clears the cookie
```

- The refresh token lives only in an **httpOnly + Secure + SameSite cookie** with
  `Path=/api/auth`. The SPA never sees it.
- The SPA holds the access token **in memory only** and gets a new one from
  `/api/auth/refresh` on load and before expiry.
- The `state` and PKCE verifier ride in a short-lived httpOnly cookie, checked at the
  callback.
- No CORS on the issuer, and no issuer origin in the CSP `connect-src`
  ([security.md](security.md)).

## Native and CLI clients

Native apps and CLIs call the issuer's token endpoint directly as public clients. They store
and rotate the refresh token per [refresh-tokens.md](refresh-tokens.md). Rust clients use
`liboidc`'s `client` feature (device flow, PKCE with a loopback listener, single-flight
refresh). Swift uses `ASWebAuthenticationSession`; Windows uses the system browser with a
loopback listener.

**Clients don't decode access tokens.** For identity and permissions they ask the server
(`GET /api/v1/me`, returning at least `sub`, a display name and the caller's grants). For
expiry they use the token response's `expires_in`.

## Service to service

- One confidential `client_credentials` client per calling service per target product,
  id `<target-slug>-<service>`. Its audience is the target's app id; its scopes are the
  target's `<target-slug>.<grant>`.
- The secret's digest lives in the Authelia config; the plain value is a Komodo secret in the
  caller's stack.
- Rotate by adding a second client (or secret), moving the caller, then removing the old one.
- The target lists the client id in `<PRODUCT>_OIDC_SERVICE_CLIENTS` and gates the route on
  the scope.

## User identity

- The principal is **`sub`**: an opaque id, the same for one person across every client and
  every product. Use it for span attributes (`enduser.id`) and signed-URL subjects.
- **Link on `(iss, sub)` only.** Never key on `preferred_username` or `email`; they change.
- Multi-user data: keep a local `users` table mapping `(iss, sub)` to a local `Id<User>`
  ([data-persistence.md](data-persistence.md)), and key owned rows on the local id. An IdP
  change or a reset `sub` is then a one-table remap.
- `sub` stays stable only while no client sets a pairwise subject (`sector_identifier_uri`)
  and the IdP's database is kept. Never set the former; the latter is backed up.

## Revocation

Local validation means a revoked or downgraded user keeps access **until their access token
expires, at most 10 minutes**. That covers sign-out, removal from a group, and a disabled
account; group changes reach the token at the next refresh. This is the accepted price of
having no per-request dependency on the issuer.

A route where 10 minutes of lag is unacceptable may introspect at the issuer for that route
only. That needs a confidential client for the server. It is optional and per route, never
the default.

## Migrating an app off the legacy SDK

- **One cut per app.** All of an app's clients move in one coordinated release. The same
  release removes `/api/login`, the old refresh and logout endpoints, any device-link broker,
  the raw-JWT `?token=` fallback and the app's own login throttle (the IdP throttles
  sign-in now).
- Clients treat a 401 or 404 from the old refresh path as **signed out**, not unreachable, so
  an old build falls to the sign-in screen.
- The `Auth: legacy-sdk` line leaves `AGENTS.md` in that release.

## Legacy: the auth service SDK

Applies only to a repo whose `AGENTS.md` says `Auth: legacy-sdk`. These rules hold until the
app migrates; nothing new is added to them.

- **Mode `sdk`.** Federated auth through the auth SDK. A missing or blank `AUTH_ADMIN_KEY`
  **fails the service closed at boot**. `none` and `proxied` are as above.
- **Registration.** App and grants declared with `#[derive(App)]` (`id = "dev.thmsn.<product>"`,
  grants `dev.thmsn.<product>.{use,read,write,admin}`), registered idempotently on boot with
  `connect_app::<Grants>(&admin_key)`.
- **Handshake.** `app_key = hex(HMAC-SHA256(admin_key, app_id))`, compared in constant time.
  `AUTH_ADMIN_KEY` is a shared Komodo secret; reuse it, never regenerate it.
- **Transport.** `connect_tls(addr, domain)` against a hostname, never a bare IP. The TLS
  settings are the service's own fields and carry its prefix:
  `<PRODUCT>_AUTH_TCP_ADDR`, `<PRODUCT>_AUTH_TCP_TLS`, `<PRODUCT>_AUTH_TCP_DOMAIN`. Only
  `AUTH_ADMIN_KEY` and `AUTH_MODE` are bare, because the SDK reads them itself. A bare
  `AUTH_TCP_*` is silently ignored. Plaintext `connect` only for loopback dev.
- **Validation.** `me()` is the live authority, behind a short-TTL (30s to 5m) token-to-user
  cache; a token's embedded claims are not trusted on their own.
- **Gating.** `user.has_grant(Grants::Admin)`; 401 vs 403 and header-less routes as above.
- **Refresh.** `login` and `refresh` return a rotating refresh token, relayed in an httpOnly
  cookie (web) or the response body (native), stored per [refresh-tokens.md](refresh-tokens.md).

## Checklist

- [ ] On OIDC, or `AGENTS.md` carries `Auth: legacy-sdk` and the repo meets the Legacy section.
- [ ] `<PRODUCT>_AUTH_MODE` declared (`oidc`/`none`/`proxied`); `oidc` refuses to boot on a
      blank issuer or audience or an unreachable JWKS; `none` is loopback-only and loud.
- [ ] Permission model declared once in code: groups `<slug>`, `<slug>-admin` (people) and
      scopes `<slug>.<grant>` (services); no string literals in route checks.
- [ ] Authelia has one client per surface (`<slug>-<surface>`), audience `dev.thmsn.<product>`
      granted implicitly, an `authorization_policies.<slug>` with default deny, and the groups.
- [ ] Public, unauthenticated `GET /api/auth/config`; no client hardcodes the issuer.
- [ ] Access tokens verified locally: RS256 pinned, `typ` `at+jwt`, exact `iss`, `aud`
      contains the app id, `exp`/`nbf`/`iat` with 60s leeway; JWKS cached with rate-limited
      `kid`-miss refetch; no per-request introspection.
- [ ] 401 (with `WWW-Authenticate`) vs 403; Admin checked in addition to Use; user routes
      refuse service tokens; service routes need an allowlisted `client_id` and the scope.
- [ ] WebSocket/SSE connections capped at the token lifetime or re-authenticated in-band.
- [ ] `?token=` only on asset/WS routes (prefer pre-signed URLs bound to `sub`); query strings
      stripped from logs.
- [ ] Web: server-side exchange as confidential `<slug>-web`; refresh token only in an
      httpOnly + Secure + SameSite cookie on `/api/auth`; SPA keeps the access token in memory.
- [ ] Native and CLI: public clients with PKCE (CLI: device flow); refresh tokens stored and
      rotated per [refresh-tokens.md](refresh-tokens.md); identity from `GET /api/v1/me`.
- [ ] Services use `client_credentials`; secrets only in Komodo.
- [ ] Users keyed on `(iss, sub)`, never username or email.
