# Refresh tokens (client storage & rotation)

## Rule

A refresh token is a long-lived bearer credential — treat it like a password. Store it in
the **strongest secret store the platform offers**, rotate it correctly (it is
single-use and non-idempotent), and never log it or put it in a URL. Access tokens are
short-lived and cheap to re-mint; refresh tokens are the durable thing worth protecting.
See [auth-integration.md](auth-integration.md) for how they are issued.

## Storage, per platform

| Platform            | Location                                            | Protection                                  |
|---------------------|-----------------------------------------------------|---------------------------------------------|
| iOS / macOS (app)   | **Keychain** (generic-password item)                | OS-encrypted; never `UserDefaults`          |
| Windows             | `%LOCALAPPDATA%\<app>\credentials`                  | **DPAPI** (`CryptProtectData`, CurrentUser) |
| Linux / BSD (CLI)   | `$XDG_STATE_HOME/<app>/credentials` (`~/.local/state`) | `0600`, `umask 077`                      |
| macOS (CLI)         | Keychain if linkable, else `~/Library/Application Support/<app>/` | Keychain > `0600` file         |
| Web                 | refresh token in an **httpOnly + Secure + SameSite cookie** | out of JS reach                     |

- The **access token** may live in memory or (web) `localStorage` as an accepted, CSP-
  compensated risk — but the **refresh token must not** be reachable by page JavaScript.
- **Atomic write** on every platform: write a temp file and rename, under a tight umask,
  so a crash never leaves a half-written credential.
- Never persist a refresh token in plaintext app settings, a dotfile in `$HOME`, shell
  history, or logs.

## Rotation

The server rotates on every `refresh`: it returns a **new** refresh token and revokes the
one presented. Reusing an already-rotated token revokes the whole family (theft
detection). Clients therefore must:

- **Persist the new token atomically and discard the old one** the moment a refresh
  succeeds — before making any request with the new access token.
- **Single-flight**: coalesce concurrent 401s into one in-flight refresh; queue other
  requests behind it and replay them with the new token.
- **Refresh proactively** ~30–60s before access-token `exp` (decode `exp`; for WebSockets,
  refresh *before* connecting if within the window, since the upgrade can't 401-replay).
- **Never blind-retry a refresh** over a dropped connection — refresh is **non-idempotent**;
  a replayed refresh trips family-revoke. Heal the connection and surface a transient
  error instead.
- On a genuine `RefreshInvalid` (not a transient/network error), **wipe stored tokens and
  force a full re-login**. Distinguish invalid-token from service-unavailable: only the
  former clears credentials.

## Transport

- Access token → `Authorization: Bearer`. `?token=` only where headers are impossible
  (asset `<img>`, WS upgrade), and prefer a subject-bound pre-signed URL even there.
- Refresh token → httpOnly cookie (web) or response body (native). **Never** in a query
  string, and stripped from logs ([observability.md](observability.md)).

## Checklist

- [ ] Refresh token in the platform's strongest store (Keychain / DPAPI / httpOnly cookie /
      `0600` file) — never `UserDefaults`, `localStorage`, or plaintext settings.
- [ ] Writes are atomic (temp + rename, tight umask).
- [ ] Rotation persists the new token atomically and discards the old before reuse.
- [ ] Concurrent 401s single-flight into one refresh.
- [ ] Proactive refresh before `exp`; WS refreshes before connecting.
- [ ] Refresh is never blind-retried (non-idempotent).
- [ ] `RefreshInvalid` wipes tokens + re-logs-in; transient errors do not.
- [ ] If you store a refresh token, you rotate it — no mint-and-ignore.
