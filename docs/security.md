# Security

## Rule

Standard primitives, applied consistently: **Argon2** for passwords, **ChaCha20-Poly1305** for
secrets at rest, **HMAC-SHA256 (constant-time)** for service handshakes, strict **CSP/security
headers** at the edge, and **defensive parsing** (SSRF allowlists, decompression caps) for any
attacker-influenced input. Auth/authorization is covered in
[auth-integration.md](auth-integration.md).

## Passwords — Argon2

Hash with `argon2` (Argon2id defaults), random salt via `SaltString::generate(&mut OsRng)`,
verify with `verify_password`. **Mitigate user-enumeration timing**: on user-not-found, hash a
dummy password anyway so the response time doesn't reveal whether the account exists.

## Secrets at rest — ChaCha20-Poly1305

Seal sensitive values (API keys, tokens) before they touch the DB:

- 32-byte master key at `<data_dir>/secret.key`, generated on first boot, **mode 0600**, reused
  across restarts.
- **Fresh random nonce per encryption**; store nonce alongside ciphertext.
- AEAD: a tampered ciphertext fails authentication (don't ignore the tag); reject wrong-length
  nonces.

## Service handshakes — HMAC

Inter-service auth derives a per-app key `HMAC-SHA256(admin_key, app_id)` and compares it with
`subtle::ConstantTimeEq` — never `==`. The root key is a shared deployment secret. Details in
[auth-integration.md](auth-integration.md).

## Edge headers / CSP

The internal Caddy proxy sets security headers on **every** response in one place
([deployment.md](deployment.md)):

- **`script-src 'self'`** — no `unsafe-inline`/`unsafe-eval`. This is the control that turns an
  XSS into a no-op for token theft; keep it strict (the built bundle is a single external
  module + modulepreloads, no inline scripts).
- `style-src 'self' 'unsafe-inline'` (React/Radix/motion need inline styles), `connect-src
  'self' wss://<host>` (**update if the hostname changes**), `img-src 'self' data: https:`,
  `frame-ancestors 'none'` + `X-Frame-Options: DENY`, `base-uri 'self'`, `object-src 'none'`.
- `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`, `Referrer-Policy:
  no-referrer`, strip `Server`.

## Token handling

- Bearer JWT in `Authorization` headers. Web stores it in `localStorage` (native clients can't
  share a cookie, and the strict CSP neutralises the XSS read vector).
- `<img>` and WebSocket upgrades can't set headers, so the token rides as `?token=` **only** on
  those routes — and the request logger **strips query strings** so it never lands in logs
  ([observability.md](observability.md)).
- Native: store in the platform keychain (Apple Keychain, service `dev.thmsn.<product>`,
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
- **Login throttling** on auth endpoints.

## Checklist

- [ ] Passwords: Argon2id + random salt + dummy-hash on user-not-found.
- [ ] Secrets sealed with ChaCha20-Poly1305; 32-byte key file mode 0600; per-secret nonce; AEAD verified.
- [ ] Service handshakes: `HMAC-SHA256(admin_key, app_id)`, constant-time compared.
- [ ] Edge CSP `script-src 'self'`; full header set; `connect-src` host kept current.
- [ ] `?token=` only on asset/WS routes; query strings stripped from logs; native tokens in keychain.
- [ ] SSRF allowlist + IP-literal rejection + manual redirect re-validation + size caps; decompression bounded.
