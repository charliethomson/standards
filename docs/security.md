# Security

## Rule

Standard primitives, applied consistently: **Argon2** for passwords, **ChaCha20-Poly1305** for
secrets at rest, **OAuth client credentials** for services (constant-time compares for any
shared secret), strict **CSP/security
headers** at the edge, and **defensive parsing** (SSRF allowlists, decompression caps) for any
attacker-influenced input. Auth/authorization is covered in
[auth-integration.md](auth-integration.md).

## Passwords — Argon2

Hash with `argon2` (Argon2id defaults), random salt via `SaltString::generate(&mut OsRng)`,
verify with `verify_password`. **Mitigate user-enumeration timing**: on user-not-found, hash a
dummy password anyway so the response time doesn't reveal whether the account exists.

## Secrets at rest — ChaCha20-Poly1305

Seal sensitive values (API keys, tokens) before they touch the DB:

- 32-byte master key from **env** (`<PRODUCT>_SECRET_KEY`, 64 hex characters), a Komodo secret
  Variable. Set but malformed ⇒ refuse to boot (a truncated paste is a mistake). Unset ⇒ the
  features that seal are off, and boot logs a warning saying so.
- **Never a file under the data dir.** The data dir is what the backup sidecar snapshots, and a
  key in the snapshot beside its own ciphertext protects nothing ([backups.md](backups.md)).
  Older services generate `<data_dir>/secret.key` on first boot; that is the pattern being
  retired, and until a service migrates, its backups can't restore sealed values without the
  key file kept separately.
- Store a `key_version` with each sealed value so a rotation can be added later without a
  migration.
- **Fresh random nonce per encryption**; store nonce alongside ciphertext.
- AEAD: a tampered ciphertext fails authentication (don't ignore the tag); reject wrong-length
  nonces.

## Service credentials

A service calling a product authenticates with OAuth `client_credentials` at Authelia and
sends the resulting access token; the target checks the client id and scope
([auth-integration.md](auth-integration.md#service-to-service)). Any shared-secret comparison
a service still does (an ingest key, the legacy auth SDK's `HMAC-SHA256(admin_key, app_id)`
handshake) uses `subtle::ConstantTimeEq`, never `==`.

## Edge headers / CSP

The internal Caddy proxy sets security headers on **every** response in one place
([deployment.md](deployment.md)):

- **`script-src 'self'`** — no `unsafe-inline`/`unsafe-eval`. This is the control that turns an
  XSS into a no-op for token theft; keep it strict (the built bundle is a single external
  module + modulepreloads, no inline scripts).
- `style-src 'self' 'unsafe-inline'` (React/Radix/motion need inline styles), `connect-src
  'self' wss://<host>` (**update if the hostname changes**; the identity provider is **not**
  in it, because the browser never calls the issuer from JS), `img-src 'self' data: https:`,
  `frame-ancestors 'none'` + `X-Frame-Options: DENY`, `base-uri 'self'`, `object-src 'none'`.
- `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`, `Referrer-Policy:
  no-referrer`, strip `Server`.

## Token handling

- Bearer access token in `Authorization` headers. Web keeps it **in memory** and the refresh
  token sits in an httpOnly cookie set by the product server
  ([refresh-tokens.md](refresh-tokens.md)). Legacy-SDK web apps may still keep the access token
  in `localStorage`, an accepted risk behind the strict CSP.
- `<img>` and WebSocket upgrades can't set headers, so the token rides as `?token=` **only** on
  those routes — and the request logger **strips query strings** so it never lands in logs
  ([observability.md](observability.md)).
- Native: store tokens in the platform keychain (Apple Keychain, service `dev.thmsn.<product>`,
  `afterFirstUnlock`; Windows DPAPI). Origin (non-secret) in UserDefaults/settings.

## Defensive parsing

Any attacker-influenced fetch or decode is bounded:

- **SSRF allowlist** for server-side fetches (remote images): host must match a configured
  allowlist (subdomain wildcards ok); **reject IP-literal hosts** (blocks `169.254.169.254`,
  loopback, private ranges); **disable automatic redirects** and re-validate every hop's
  `Location` before following; cap redirects and **cap body bytes**.
- **Decompression caps**: bound zlib/zTXt decompression (e.g. 8 MiB) and detect overflow by
  reading one byte past the cap rather than silently truncating. Build the HTTP client without
  auto-decompression (`gzip`/`brotli`/`deflate` off).
- **Login throttling** on auth endpoints. On OIDC the identity provider throttles sign-in
  (Authelia's regulation) and products have no login endpoint; a legacy-SDK app throttles its
  own `/api/login`.

## Checklist

- [ ] Passwords: Argon2id + random salt + dummy-hash on user-not-found.
- [ ] Secrets sealed with ChaCha20-Poly1305; 32-byte key from env (never under the data dir), `key_version` stored; per-secret nonce; AEAD verified.
- [ ] Services authenticate with `client_credentials`; any shared-secret compare is constant-time.
- [ ] Edge CSP `script-src 'self'`; full header set; `connect-src` host kept current.
- [ ] `?token=` only on asset/WS routes; query strings stripped from logs; native tokens in
      keychain; web access token in memory, refresh token in an httpOnly cookie.
- [ ] SSRF allowlist + IP-literal rejection + manual redirect re-validation + size caps; decompression bounded.
