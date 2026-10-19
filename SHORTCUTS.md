# Shortcuts

How to answer the questions that get asked about **standards** over and over: what's live,
which commit the server is serving, where the logs are. One command each, so nobody
re-derives them from the workflows and compose files every session.

**If you are an agent:**

- **Read this before any environment recon.** Question about the running system (prod,
  deploys, versions, logs, metrics, health)? The answer may already be below.
- **Write to it after any environment recon.** If answering took more than a couple of
  commands and the answer is durable, append an entry in the same turn, before you report
  back.
- **Record the route, not the reading.** The endpoint, command, or mechanism; never
  "prod is on 1.4.212" or "deployed at 14:32".
- **Fix what's broken.** Ran a shortcut and it failed or the output changed? Correct the
  entry now. Bump `_Verified:_` when a shortcut works.
- **No secrets.** Reference an env var or Komodo Variable; never paste the value.

Keep it under ~15 entries. Full convention: [`docs/shortcuts.md`](docs/shortcuts.md).

---

_Nothing recorded yet: the server is not deployed. The go-live task should add "what
commit is live" (`/manifest.json`'s `commit` against `git rev-parse HEAD`) and where the
logs are._
