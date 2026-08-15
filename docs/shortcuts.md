# Shortcuts

Every repo keeps a **`SHORTCUTS.md`** at its root: a short, agent-maintained list of *how to
answer the operational questions that get asked over and over*.

> "Is it up to date in prod?" should cost one command, not fifteen.

It is the antidote to a specific failure: an agent asked a routine question about the
running system — which URL is prod, what version is live, where do the logs go — and
rediscovers the answer from scratch by reading workflows, compose files, and Caddyfiles.
That recon is expensive, it happens every session, and it produces the same answer every
time. Write the answer down once.

## The rule

**Read it before doing environment recon. Write to it after doing environment recon.**

- **Read** — when a question is about the *running system* rather than the source (prod,
  deploys, versions, logs, metrics, health, registries, dashboards), open `SHORTCUTS.md`
  first. If the entry is there, run it and answer.
- **Write** — after you answer such a question the slow way, append the shortcut. The bar:
  it took **more than a couple of commands** (or reading more than one config file), and
  the answer is **durable**. Do it in the same turn, before reporting back.

## Record the route, not the reading

This is what keeps the file from rotting. A shortcut records **how to find out**, never
**what the answer was today**.

| Record this | Not this |
|---|---|
| `curl -s https://someproduct.dev.thmsn.dev/api/build \| jq -r .version` | "prod is on 1.4.212" |
| "Watchtower polls every 5m, so a push lands a few minutes later" | "deployed at 14:32" |
| "logs: Grafana → Explore → Loki, `{service="dev.thmsn.someproduct.server"}`" | a pasted log line |

Versions, timestamps, and current state go stale within the hour. Endpoints, commands,
hostnames, and mechanisms stay true for months.

## What goes in

Operational routes, keyed by the question as you'd actually ask it:

- **Prod/staging URLs** and which is which.
- **Version/health/build endpoints** — the one command that reports what's live.
- **The deploy mechanism and its lag** — what makes a push become a running container, and
  how long it takes (the difference between "pushed" and "deployed" is the single most
  common misread).
- **Where logs, metrics, and dashboards live** — the concrete query, not "check Grafana".
- **Registry/artifact locations** — image names, install manifests.
- **Local dev incantations** that aren't obvious from the README.

## What stays out

- **Anything already documented** in `standards/` or this repo's own docs — link to it
  instead, or put it there and link. `SHORTCUTS.md` is an index of routes, not a second
  copy of the standards.
- **Secrets** — no tokens, passwords, or credentials. Commands may *reference* an env var
  or a Komodo Variable; they never contain the value. Same rule as
  [`deployment.md`](deployment.md).
- **One-off values** — see above.
- **Explanations of the code.** If it's about how the software works, it belongs in a doc.

## Format

One `##` heading per question, phrased the way it gets asked, then the command, then what
the answer means:

````markdown
## Is it up to date in prod?

```sh
curl -s https://someproduct.dev.thmsn.dev/api/build | jq -r '.version, .commit'
```

Compare against local: `git rev-list --count HEAD` (the derived
[version](standards/docs/versioning.md) is the commit count). Deploys are Watchtower-polled
every ~5m, so a just-pushed commit takes a few minutes to appear — "pushed" ≠ "live".

_Verified: 2026-08-13_
````

The `_Verified:_` date is the trust signal. When you run a shortcut and it works, bump the
date. When it fails or the output shape changed, **fix the entry in the same turn** — a
wrong shortcut is worse than no shortcut, and you are already in the one context that knows
the new answer.

## Keep it small

Target **≤15 entries**, skimmable in under a minute. It is read at the start of many
sessions, so it earns its size in saved recon or not at all. When an entry grows into
prose, promote it into a real doc and leave a one-line pointer behind.

## Scaffolding

`standards install` writes the file from
[`../templates/SHORTCUTS.md`](../templates/SHORTCUTS.md), and `standards sync` backfills it
for repos installed before this convention existed. Neither ever clobbers an existing one.

The root `AGENTS.md` stub carries the read/write rule — that stub is what agents actually
load, which is why the trigger lives there and not only here. Repos whose `AGENTS.md`
predates this need the pointer block from
[`../templates/consuming-repo-AGENTS.md`](../templates/consuming-repo-AGENTS.md) added by
hand (`install` won't overwrite an existing `AGENTS.md`).

Commit it. The whole point is that the next session — and the next agent — starts with the
answer already in hand.

## Checklist

- [ ] `SHORTCUTS.md` exists at the repo root and is committed.
- [ ] Root `AGENTS.md` points at it with the read-before / write-after rule.
- [ ] Every entry is a route (command/endpoint/mechanism), not a captured value.
- [ ] Every entry carries a `_Verified:_` date.
- [ ] No secrets; no duplication of `standards/` content.
- [ ] ≤15 entries.
