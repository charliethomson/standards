# PROGRAM: stash

slug: `stash` (derived from the repo name — single-repo program, no program label of its own)
tracker: vision project **STASH** (`charliethomson/stash`)
repo: `~/git/stash`, remote `charliethomson/stash` (private)
started: 2026-09-21 (ultron took ownership)

## SYSTEM CONTEXT

stash is the file server for the NFS tree on the **fs box** (`192.168.0.62`), replacing
copyparty. Rust/poem API + React client in one image, central auth grants, SQLite search
index, thumbnail cache. Komodo stack on **fs**, not dev.

```
browser ──HTTPS──► fs Caddy (homelab repo, fs/caddy/, host net)
                     stash.fs.thmsn.dev → 127.0.0.1:8080
                     ▼
                   proxy (caddy:2-alpine, stash stack — owns the CSP)
                     ▼
                   api   (ghcr.io/charliethomson/stash-api:main, :8080)
                         /nfs/files + /nfs/secure bind-mounted; /data volume
                   watchtower (label-scoped, polls GHCR)
```

Deploys go through **Komodo + Periphery** only. Never `docker compose` in a checkout.
Push to `main` → `image.build.yml` publishes `:main` → Watchtower rolls it, or press
Deploy in Komodo.

Crate layering (deps inward only): `stash-core` → `stash-db` / `stash-storage` /
`stash-thumbs` → `stash-engine` → `stash-api`. `stash-storage` is the **only** crate that
touches `/nfs`; that is what makes path safety reviewable.

## LOCKED DECISIONS — do not relitigate

1. **Listings are `readdir` + `stat`, always. They never consult the index.** The index
   exists for search only and is deletable at any time. A stale index costs a search
   result, never a wrong directory.
2. **A crawl that sees a missing sentinel aborts and deletes nothing.** Missing
   `.stash-root` → root is degraded → 503 for **that root only**, never the container.
   No writes to a degraded root, ever.
3. **Uploads stage inside the destination root** (`<root>/.stash/tmp/<id>.part`) so finish
   is an atomic `rename(2)` within one filesystem. Never stage under `/data`.
4. **Thumbnail decode caps are not tuning knobs** — max source bytes, max decoded pixels,
   one concurrent decode per CPU. Thumbnailing is the only place stash parses untrusted
   bytes and the tree is written to by the *arr stack from the internet.
5. **Container uid is `3000:3000`**, baked into the image *and* repeated in
   `prod.compose.yml`. Settled in STASH-2. The `stash-data` volume inherits `/data`'s
   ownership from the image, so a compose-only override breaks the data dir.
6. **Missing grant → 404, not 403.** A 403 on `/secure/tax-2019` confirms the path exists.
7. **`info.version` in `api/openapi.json` is pinned to `v1`**, not the build version.
   `version::VERSION` derives from commit count, so an unpinned spec changes on every
   commit and the drift gate goes red for changes touching no route. Two tests hold the
   pin. Do not "fix" this.
8. **The 80% coverage floor does not move.** Currently measuring 85.87%.
9. **Path is a query parameter** (`?path=…`), not `/files/{path...}`. OpenAPI has no
   catch-all path parameter, so a wildcard route cannot appear in the contract the client
   is generated from and CI diffs. Also: a query value has exactly one decoding.
10. **Commit straight to `main`, no PRs.** `/thmsn-standards-review` before every push.
11. **No `/api/login`** — auth is central. (STASH-9 is how a *browser* gets a token; that
    is an open decision, see below, and it is the user's to make.)
12. **Not a share service, not a mount, not a sync client, not a storage system.**

## GOAL

Stash running on the fs box, serving the real NAS tree under central auth, with copyparty
deleted and `fs/` in the homelab repo reduced to a Caddyfile. That last step is the reward:
the fs box stops being the only non-Komodo deploy path in the fleet.

## READ FIRST

| path | what |
|---|---|
| `~/git/stash/DESIGN.md` | the product and the reasoning. The invariants above come from here. |
| `~/git/stash/deploy/README.md` | everything that has to exist outside the repo |
| `~/git/stash/AGENTS.md` | repo working rules |
| `~/git/stash/deploy/komodo/sync/*.toml` | the Komodo resources + why servers.toml declares nothing |
| `~/git/homelab/CLAUDE.md` | the deployment model, the config_files/post_deploy gotchas |
| `~/git/homelab/fs/caddy/Caddyfile` | the `stash.fs.thmsn.dev` vhost (already pushed) |
| `~/git/stash/standards/docs/` | binding. `security.md` for the decode caps. |

## BACKLOG

| task | state | what | owner |
|---|---|---|---|
| STASH-1 | landed | homelab wiring: AdGuard rewrite, fs Caddy vhost, Prometheus scrape | done |
| STASH-2 | landed | container uid settled: 3000:3000 | done |
| STASH-3 | **DONE** | M0 met: `:main` in GHCR, container boots and serves on dev | closed 2026-09-21 |
| STASH-10 | landed | pre-push gates + standards review on the 4 unpushed M1 commits, then push | agent |
| STASH-4 | **DONE** | M1 verified live: browse, download, Range 206, sentinel→503 for that root only | closed 2026-09-21 |
| STASH-9 | **DONE** `088a7b2` | sign-in: fleet login + ONE opaque session cookie (see decision 15) | closed 2026-09-21 |
| STASH-5 | **dispatched** 2026-09-21 | M2: writes — resumable uploads, mutations, janitor, CSRF Origin check | agent |
| STASH-6 | queued | M3: the search index | agent |
| STASH-7 | queued | M4: thumbnails and preview | agent |
| STASH-8 | queued | M5: cutover — flip fs.thmsn.dev, delete copyparty | agent + 🔒 human gate on the flip |

### The M0 gate (STASH-3) — why it is not mine

Everything left in M0 lives outside the repo:

- Woodpecker repo + **trusted volumes** grant + `weekly-audit` cron — server-side per-repo
  settings with no in-repo representation.
- The Komodo sync registration and its Variables.
- `sudo touch /nfs/files/.stash-root /nfs/secure/.stash-root` on fs (`c` has no
  passwordless sudo there).

**The komodo MCP cannot do any of it**, for two independent reasons, both verified
2026-09-21:

1. Its credentials are dead — `POST https://komodo.dev.thmsn.dev/read` returns
   `401 Invalid user credentials | You have 4 attempts remaining` with the key/secret in
   `~/git/homelab/mcp/.env` (both 44 chars, so present, not empty). **Stopped after one
   direct attempt** — there is a lockout counter and burning it is worse than being
   blocked. Fix: Komodo UI → Settings → API Keys → new key, update `KOMODO_API_KEY` /
   `KOMODO_API_SECRET` in `~/git/homelab/mcp/.env`.
2. Even with a live key the MCP exposes only **read** and **execute** request types
   (`mcp/komodo/server.py`). Creating a Sync resource or a Variable is a **write**-type
   request and there is no tool for it. So the sync registration and the Variables are UI
   work regardless of the key.

Filed the MCP key/coverage gap rather than working around it — **HOMELAB-7**.

### Discovered while reading, worth knowing

- **`ed92d36` (the stash Prometheus scrape) is pushed**, contrary to the ordering note in
  STASH-1's comment which said it should not be pushed ahead of the stack. Verified:
  `git merge-base --is-ancestor ed92d36 origin/main` → yes, and `origin/main` is `922a722`.
- **Consequence, and it is good news:** Prometheus has `job="stash"` and is scraping it, so
  the monitoring deploy **succeeded**, so the config-renderer's assert on
  `STASH_METRICS_TOKEN` passed, so **that Komodo Variable already exists with a non-empty
  value.** Inference, not a direct read (no Komodo access) — worth the user confirming
  while they are in the UI, but one item likely already off STASH-3's list.
- `up{job="stash", instance="fs.thmsn.local"}` = **0** right now, so `infra-target-down`
  is firing for it. Expected and documented at `monitoring/prometheus/config/prometheus.yml:127`.
  Clears when the stack boots. Do **not** add `stash` to the alert's negative matcher —
  STASH-1 deliberately left it out, the matcher is negative so a new job cannot be added
  without coverage.
- The **AdGuard rewrite for `stash.fs.thmsn.dev` is almost certainly still not live.**
  `dns/AdGuardHome.yaml.template` has it, but the renderer's `[ -f ]` guard only seeds the
  *initial* config — AdGuard owns `live/conf/AdGuardHome.yaml` after first start. So it is
  UI work (Filters → DNS rewrites → Add). Already documented in `dns/README.md`; STASH-1's
  comment lists it as still-manual and nothing since says it was done.

## DISPATCH LOG

| task | state | notes |
|---|---|---|
| STASH-3 | blocked 🔒 | handed to the user as a numbered click list 2026-09-21. Waiting. |
| STASH-10 | landed | gates green, coverage 85.87%, 3 standards fixes. `origin/main` was `89193c8`. |
| STASH-12 | landed (in_review) | stack moved fs → dev. stash `ccf8bec`, homelab `09ce2ae`, both pushed. Verified. |
| STASH-13 | blocked 🔒 | first image build failed. Two undocumented Woodpecker secrets (`ssh_key`, `ghcr_token`). Waiting on the log. |
| STASH-11 | queued | no `/api/metrics` route; Prometheus already scrapes it. Has a crate-choice decision in it. |
| STASH-9 | decided, not dispatched | see LOCKED DECISIONS 13-15. |
| STASH-9 | blocked 🔒 | tradeoff to be laid out for the user. **Do not pick one and start building.** |

## RESUME POINT

Nothing dispatched yet. Next actions, in order:
1. Waiting on the user for the STASH-3 click list (Woodpecker, Komodo sync, sentinels).
2. Dispatch STASH-10 (pre-push gates + standards review + push) — runs fine while blocked.
3. Put the STASH-9 tradeoff in front of the user and wait for the call.


## LOCKED DECISIONS, added 2026-09-21 (second batch)

13. **The stack runs on dev (`192.168.0.193`), not the fs box.** DESIGN.md's old argument was
    factually wrong: fs was an NFS client of `192.168.0.228` exactly like dev, so neither box
    was closer to the bytes. Mounts on dev are `/mnt/truenas/Files` and `/mnt/truenas/Secure`
    — capital `Secure`, though the export is `.secure`. Loopback port is **8085**, not 8080;
    qBittorrent owns 8080 on dev. Ingress is `gateway/Caddyfile`, hostname
    `stash.dev.thmsn.dev`, covered by the `*.dev.thmsn.dev` wildcard so no AdGuard rewrite.
14. **Sign-in is the ordinary fleet login, NOT a device-link flow.** `POST /login` → the SDK's
    `Login` over TCP → JWT + rotating refresh. `X-Client: web` puts the refresh token in an
    `HttpOnly; Secure; SameSite=Strict` cookie on stash's own origin. Copy
    `vision/server/api/src/refresh_cookie.rs`. DESIGN.md's "no `/api/login`" means no password
    table, not no login route, and must be reworded — it has already misled one pass.
15. **ONE cookie, not two — corrected 2026-09-21 after STASH-9 shipped.** The header-less
    routes (download, thumb, video) are authenticated by a single opaque `stash_session`
    cookie, `Path=/api/v1`, `HttpOnly; Secure; SameSite=Lax`, `Max-Age` 30 days, backed by a
    persisted row holding the subject, the access JWT **and** the refresh token. The browser
    gets **no tokens at all**; stash rotates server-side, lazily, single-flight.

    There is **no separate refresh cookie** and page JS must not rotate anything. My original
    comment asked for vision's `refresh_cookie.rs` *as well*, which was an inconsistency in my
    own brief: a login yields exactly one rotating, single-use refresh token, so the browser and
    the server cannot both hold it — two independent rotators trip auth's family-revoke on the
    second page load (`standards/docs/refresh-tokens.md`: "a replayed refresh trips
    family-revoke"). The implementer flagged this before writing code and was right.
    `me()` is still the live authority on every request via the 30s cache, so revocation is
    unaffected. **Not** a pre-signed URL and **not** the access JWT in
    a cookie: both are short-lived and a resumed multi-GB download is a new request made hours
    later by the browser's own download manager, with no JS in the loop to re-mint anything.
    libsign is therefore not used anywhere in this product. `SameSite=Lax` means **M2 needs an
    `Origin`/`Referer` check** on mutations.

## STATE AS OF 2026-09-21 ~15:10

**`woodpecker-cli` works from this machine** — `/opt/homebrew/bin/woodpecker-cli`, context
`default` → `https://woodpecker.dev.thmsn.dev`, already authenticated, no token needed. Use it
rather than asking for pasted logs:
`woodpecker-cli --disable-update-check pipeline {ls,ps,log show} charliethomson/stash [N] [step]`

**STASH-13 root cause found and fixed in `07beb5b`:** `protoc` missing. `auth-sdk` pulls in
`auth-proto`, whose `build.rs` uses prost-build, and `rust:1-bookworm` does not ship it.
`standards/templates/docker/Dockerfile.rust` ships `openssh-client protobuf-compiler` as one
line; this repo took the SSH half only, in `Dockerfile:19` and `rust.ci.yml:74`/`:114`. Scaffold
bug, not an M1 regression. **Not** the secrets — `ssh_key` works, proven by `auth-proto` being
fetched. `rust-advisories` showing `failure` is `failure: ignore` and not implicated.

Pipeline 2 running on `07beb5b`, watched by a background shell.

Sentinels confirmed on dev (`nobody:root`, mode 0777 — existence is all that is checked).
8085 confirmed free.

### Dispatched, both in worktrees, both concurrent

- **STASH-11** — LANDED `7fe0216`, verified. Crate choice was **not** a user decision after all:
  `standards/docs/observability.md` prescribes `prometheus-client` with
  `application/openmetrics-text; version=1.0.0; charset=utf-8` and explains why at length. So the
  declared `metrics` + `metrics-exporter-prometheus` pair gets swapped; `subtle` stays.
- **STASH-9** — still running. Sign-in + the `stash_session` cookie; first real use of `stash-db`.
- **STASH-14** — LANDED `9f62c22`. Auth transport was plaintext to a TLS-only listener, and a bare
  IP. `connect_tls(addr, domain)`, TLS default on, two tests hold it.

## WAITING ON THE USER

1. Run the Komodo sync once the image exists — redeploys `gateway`, `monitoring`, `fs-caddy`.
2. Deploy the stash stack, then STASH-4 can finally be verified live.
3. Optional: rotate `WOODPECKER_AGENT_SECRET` — I printed it into a transcript by accident
   (a redaction pattern missed uppercase `SECRET` in `~/.config/woodpecker/agent.env`).
4. One-line amend owed to STASH-2: the sentinels came out `nobody:root`, so root **is** squashed
   on that export. Non-root uids still pass through unmapped, so 3000:3000 and everything built
   on it stands; only the "no squashing in either direction" phrasing was too strong.

## OPERATIONAL LESSON, 2026-09-21 — Woodpecker `killed` means superseded

A step in state **`killed`** is almost always a pipeline that a newer pipeline on the same branch
cancelled, not a kernel OOM. The timestamps prove it: P3's build stopped at `1790004511` and P4
started at `1790004510`.

I misread `killed` as an OOM kill (`rust.ci.yml`'s own comment about agent-2's OOM history made it
plausible) and **restarted the superseded pipeline**. That created P5 on the *older* commit
`ea7ef67`, whose image step then pushed `:main` over P4's newer `7fe0216` image, regressing the
deployed tag to the pre-metrics build. Recovered by stopping P5 and restarting P4 as P6.

**Rule: never restart a pipeline a newer push has superseded. Restart the newest one.** And check
`pipeline ps <n>` timestamps against the next pipeline's start before diagnosing a kill at all.

## PROCESS LESSON, 2026-09-21 — read the issue while an agent is running

STASH-9's implementer flagged a real design conflict in my brief in its **opening** boundary
comment, exactly as instructed, and then proceeded after ~45 minutes of silence with "Nobody
objected". It happened to be right. But I had dispatched and then not looked at the workitem until
the task reported back, so the gate I built into the brief did nothing.

**When a brief says "comment when you hit something a human must decide", poll that issue.** A
boundary comment nobody reads is not a gate.

## PRODUCTION-ONLY CODE PATHS — three boot failures in a row

Every failure this milestone existed only in the container, and none was caught by 163 tests:

1. `protoc` missing (build, not runtime) — STASH-13
2. plaintext auth dial to a TLS-only listener — STASH-14. Locally there is no auth to dial.
3. `nest("/", api)` + `nest("/", static)` — STASH-16. `web_dir` is `Some` **only** in the image;
   every test passed `None` and `cargo run` leaves `STASH_WEB_DIR` unset for a Vite dev server.

And STASH-9 added a fourth: **the server now fails to boot if `/data` is not writable by uid
3000.** (Checked: the Dockerfile chowns `/data` before `VOLUME`, so the named volume inherits
3000:3000.)

**Rule for the rest of this program: anything touching the mount map, the auth transport or
`/data` needs a test that runs the container's configuration, not the default one.**

## M0 + M1 COMPLETE — 2026-09-21

stash is live at `stash.dev.thmsn.dev`, serving the real NAS tree, behind central auth.
Deployed `088a7b2` / version `0.0.15`.

Verified in production (not against a fake): sign-in, browse, download with **no
`Authorization` header** (cookie only), `Range` → `206` with `content-range: bytes 0-10/26` and
a stable `ETag`, and the sentinel invariant exercised for real — `/mnt/truenas/Secure/.stash-root`
moved aside took `secure` to 503 while `files` kept serving and `/health` stayed `200`.

Open, non-blocking: STASH-17 (`weekly-audit` cron), HOMELAB-8 (CI agent disk 100% full, which is
why coverage could not run on `088a7b2`), VIS-4, STD-3.

Next after M2: M3 (index) and M4 (thumbs), which are independent of each other. **Do not dispatch
them concurrently with M2** — all three add `stash-db` migrations and would collide on the
`0003_*.sql` filename, and M2 is already touching routes, db and the web client.

## RESUMED 2026-09-24 — "work through all the stash tasks, I'm trying to get off copyparty"

Reconciled against vision + the box:
- Live: 0.0.16 = `e8665d7` (M2). HEAD `55f7eae` + docs commit did not build an image (path filter), correct.
- STASH-18 verified on dev (Periphery clone at 55f7eae, `.env` mtime 07:55:51.5 then api+proxy
  both restarted 07:55:52, no reboot) → **done**.
- STASH-9 → **done** (live sign-in was verified on 0.0.15, see M0+M1 section).
- STASH-5 stays in_review 🔒: needs Charlie signed in for the real interrupted big upload + `stat`
  on the NAS (uid 3000). Not an agent task.
- Docs commit pushed: DESIGN.md M5 step 5 (every fs-box reference) + SHORTCUTS.md first entries.
- ssh to the box works as `dev.thmsn.dev` (alias), not the bare IP.

### LOCKED DECISIONS, third batch

16. **Integration for parallel M3/M4:** each agent works in its own worktree and commits there,
    does **not** push. Ultron merges M3 first, rebases M4 onto it, regenerates the contract and
    the web client, re-runs gates, pushes. Migration numbers pre-assigned: **M3 = `0004_*`,
    M4 = `0005_*`** (M4 should need none; the thumb cache is files under `/data/thumbs`).
17. **The thumb route follows decision 9**: `GET /api/v1/roots/{root}/thumb?path=…&size=…`, not
    DESIGN.md's `/thumb/{path...}`. `size` is one of a small fixed set so the cache key space is
    bounded.
18. Search results never reveal a root the caller lacks the grant for (decision 6 applies).

| task | state | notes |
|---|---|---|
| STASH-18 | landed | done 2026-09-24 |
| STASH-9 | landed | done 2026-09-24 |
| STASH-5 | blocked 🔒 human-gated | real 8GB interrupt + NAS stat, needs Charlie signed in |
| STASH-6 | landed | 331dfb5 pushed 2026-09-24, verified gates; in_review until first real crawl + live search
| STASH-7 | landed | 4285b06 pushed 2026-09-24 (rebased on M3), gates verified; in_review until photos browsable live |
| STASH-8 | queued | after M3+M4. Dashboard + homelab prep can land inert; the flip and copyparty removal are 🔒 |
| STASH-17 | landed | cron created via woodpecker-cli 2026-09-24, done |
| HOMELAB-9 | queued | Grafana dashboard. Agent, after STASH-6+7 land (needs their metric names) |
| HOMELAB-10 | blocked 🔒 human-gated | the flip: gateway vhost alias (agent can prep) + AdGuard UI rewrite. Gate: STASH-5 verified |
| HOMELAB-11 | blocked 🔒 human-gated | sudo on fs: disable copyparty, vacuum journal |
| HOMELAB-12 | queued | remove fs from homelab. Agent preps commits; **push gated** until 10+11 done |

M5 was split 2026-09-24 into HOMELAB-9..12, label `stash-cutover`, all `--blocks STASH-8`.
origin.rs compares against the request's own Host → the fs.thmsn.dev alias needs no stash change.

### 2026-09-24 ~15:30 — M3 + M4 live
- 0.0.19 = M3 (331dfb5), 0.0.20 = M4 (4285b06), 3b5e7af (crawl 1h) pushed after P12 green (cov 92.78%).
- First crawl: files 59,541/119s, secure 38,590/77s, pace-bound. Default interval → 1h.
- STASH-6/7 in_review, waiting only on Charlie clicking search + photos in a browser.
- Homelab agent dispatched: HOMELAB-9 dashboard (main), HOMELAB-10 vhost alias inert (main), HOMELAB-12 on branch `stash-cutover/remove-fs` (never main).
- Never edit an applied migration (sqlx checksum → boot failure).

### 2026-09-24 ~15:45 — homelab prep landed; everything left is human-gated
| HOMELAB-9 | landed | a79fbc8 on main, 13/13 metrics used, 0 unknown. in_progress until Charlie runs the sync + eyeballs Grafana |
| HOMELAB-10 | blocked 🔒 human-gated | vhost alias 1cd20f6 on main (inert, validated with the gateway's own image). Left: AdGuard UI rewrite. Gate: STASH-5 |
| HOMELAB-11 | blocked 🔒 human-gated | sudo on fs |
| HOMELAB-12 | landed | branch stash-cutover/remove-fs (9 commits, greps clean). Merge only after 10+11; check the sync's pending deletes first |
Notes: 5 Periphery servers → 4 (builder exists). Untracked fs/.env + fs/live/* secrets in the laptop checkout: delete by hand after the merge, never `git add -A`. The copyparty salt got printed into the agent's session log; it dies with copyparty.
RESUME POINT: nothing for agents. Waiting on Charlie for: STASH-5 upload, STASH-6/7 browser checks, homelab sync, then the flip sequence.
| STASH-19 | landed | e35bd13 pushed, audit green, done |
