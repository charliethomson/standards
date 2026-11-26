# Refresh tokens (client storage & rotation)

## Rule

A refresh token is a long-lived bearer credential: treat it like a password. Store it in the
**strongest secret store the platform offers**, rotate it correctly (it is single-use and
non-idempotent), and never log it or put it in a URL. Access tokens are short-lived (10
minutes) and cheap to re-mint; refresh tokens (30 days) are the durable thing worth
protecting.

Refresh tokens are issued by the identity provider (Authelia) when a client asks for the
`offline_access` scope. See [auth-integration.md](auth-integration.md) for the flows. Who holds
the token depends on the client:

- **Web:** the product server, as the confidential client, holds it in a cookie the page can't
  read. The browser never handles it directly.
- **Native apps and CLIs:** the client itself, as a public client, straight from the issuer's
  token endpoint.

## Storage, per platform

| Platform            | Location                                            | Protection                                  |
|---------------------|-----------------------------------------------------|---------------------------------------------|
| iOS / macOS (app)   | **Keychain** (generic-password item)                | OS-encrypted; never `UserDefaults`          |
| Windows             | `%LOCALAPPDATA%\<app>\credentials`                  | **DPAPI** (`CryptProtectData`, CurrentUser) |
| Linux / BSD (CLI)   | `$XDG_STATE_HOME/<app>/credentials` (`~/.local/state`) | `0600`, `umask 077`                      |
| macOS (CLI)         | Keychain if linkable, else `~/Library/Application Support/<app>/` | Keychain > `0600` file         |
| Web                 | set by the product server in an **httpOnly + Secure + SameSite cookie**, `Path=/api/auth` | out of JS reach |

- The web **access token** lives in memory only, re-minted from `/api/auth/refresh` on page
  load. The **refresh token must never** be reachable by page JavaScript, so a web client
  never calls the issuer's token endpoint itself.
- **Atomic write** on every platform: write a temp file and rename, under a tight umask,
  so a crash never leaves a half-written credential.
- Never persist a refresh token in plaintext app settings, a dotfile in `$HOME`, shell
  history, or logs.

## Rotation

Every refresh returns a **new** refresh token and revokes the one presented. Some issuers also
revoke the whole token family when a rotated token is replayed; don't depend on that either
way. Clients must:

- **Persist the new token atomically and discard the old one** the moment a refresh
  succeeds, before making any request with the new access token.
- **Single-flight**: coalesce concurrent 401s into one in-flight refresh; queue other
  requests behind it and replay them with the new token.
- **Refresh proactively** 30 to 60s before the access token expires. Take the expiry from the
  token response's `expires_in`; clients don't decode access tokens. For WebSockets, refresh
  *before* connecting if within the window, since the upgrade can't 401-replay.
- **Never blind-retry a refresh** over a dropped connection. Refresh is **non-idempotent**: the
  first attempt may have rotated the token already. Heal the connection and surface a
  transient error instead.
- On a genuine **`invalid_grant`** from the token endpoint (or a 401 from a web server's
  `/api/auth/refresh`), **wipe stored tokens and send the user through sign-in again**.
  Distinguish invalid-token from unreachable: only the former clears credentials. A refresh
  token can become invalid at any time (expiry, sign-out elsewhere, removal from the app's
  group), so this path is normal, not exceptional.

## Transport

- Access token → `Authorization: Bearer`. `?token=` only where headers are impossible
  (asset `<img>`, WS upgrade), and prefer a subject-bound pre-signed URL even there.
- Refresh token → the issuer's token endpoint (native, CLI), or an httpOnly cookie between the
  browser and its product server (web). **Never** in a query string, never in a response body
  to page JavaScript, and stripped from logs ([observability.md](observability.md)).

## Legacy apps

A repo on the legacy auth SDK (`Auth: legacy-sdk`, see
[auth-integration.md](auth-integration.md#legacy-the-auth-service-sdk)) gets its refresh token
from its own server's `login`/`refresh` routes, and calls the invalid case `RefreshInvalid`.
Storage, rotation and transport rules above apply unchanged, except that its web client may
keep the access token in `localStorage` as an accepted, CSP-compensated risk.

## Checklist

- [ ] Refresh token in the platform's strongest store (Keychain / DPAPI / httpOnly cookie /
      `0600` file); never `UserDefaults`, `localStorage`, or plaintext settings.
- [ ] Web: the product server holds the refresh token in an httpOnly cookie; page JS never
      sees it and never calls the issuer; the access token lives in memory.
- [ ] Writes are atomic (temp + rename, tight umask).
- [ ] Rotation persists the new token atomically and discards the old before reuse.
- [ ] Concurrent 401s single-flight into one refresh.
- [ ] Proactive refresh before expiry (from `expires_in`); WS refreshes before connecting.
- [ ] Refresh is never blind-retried (non-idempotent).
- [ ] `invalid_grant` wipes tokens and goes back to sign-in; transient errors do not.
- [ ] If you store a refresh token, you rotate it; no mint-and-ignore.
