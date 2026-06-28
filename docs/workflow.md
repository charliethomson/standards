# Workflow

## Rule

**Single developer, single user. Commit straight to `main`.**

There is no review gate, no release train, no long-lived feature branches by default.
`main` is always the source of truth and is continuously deployed. Optimise for one person
moving fast, not for coordinating a team.

## What this means in practice

- **Commit to `main`.** Push directly. Don't open a PR to yourself for routine work.
- **Branch only when there's a real reason** — a risky spike you might throw away, or work
  you want CI to exercise in isolation before it lands. Merge it back fast; don't let
  branches age.
- **Continuous deploy.** Pushing to `main` builds new `:main` images, and Watchtower (or a
  manual Komodo deploy) rolls them out. Every commit is a potential production build, so
  keep `main` working — a broken commit is a broken deploy.
- **Versions are for debugging, not releases.** Because every commit can ship, the
  [version](versioning.md) is a monotonic `MAJOR.MINOR.<commit count>` whose job is to
  answer *"which commit is the user running?"* — not to mark a curated release. Tag
  `vMAJOR.MINOR` only at genuinely meaningful boundaries.
- **CI still runs on PRs if you open one.** The build/test workflows trigger on both `push`
  to `main` and `pull_request` (PRs build but don't publish), so a branch gets the same
  checks — there's just no human approval step.

## Why

The whole fleet exists to serve its author. Process designed for teams (mandatory review,
release branches, changelogs, approvals) is pure friction here. The safety that review
would provide comes instead from **automation**: derived versions can't drift, CI gates
every commit ([ci-cd.md](ci-cd.md)), coverage floors hold ([testing.md](testing.md)), and
`libbuildinfo` makes any running binary traceable to an exact commit. Trust the machine,
not the ceremony.

## Checklist

- [ ] `main` is the default and is committed to directly.
- [ ] No self-imposed PR-approval gate.
- [ ] Branches, if any, are short-lived and merged quickly.
- [ ] `main` is kept deployable — a green CI is the bar for "done".
- [ ] Meaningful boundaries marked with a `vMAJOR.MINOR` tag; nothing else hand-versioned.
