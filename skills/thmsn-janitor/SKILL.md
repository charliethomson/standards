---
name: thmsn-janitor
description: Find and clean up the git worktrees and cargo target dirs that agents leave behind across ~/git — stale subagent worktrees, orphaned worktree dirs, per-worktree target dirs eating tens of GB. Use for "clean up worktrees", "what's eating my disk", "find claude's mess", "prune stale worktrees", or after a program that fanned out implementer subagents.
---

# thmsn-janitor — clean up after agents

Subagents run in their own worktrees, and each one that builds Rust grows its own `target/`.
They're rarely removed, so they pile up across every repo in `~/git`. `bin/janitor` finds them
and decides what's safe to delete. This skill runs it and gets the user's go-ahead.

## 1. Find the script

First of: `janitor` on `PATH`, `standards/bin/janitor` in the current repo, or
`~/git/standards/bin/janitor`.

## 2. Scan

```sh
janitor scan
```

Read-only. It walks every repo under `~/git` (`--root` to change, repeatable) and prints each
linked worktree with its age, size, cargo target size and a verdict. A worktree is marked
`remove` only when deleting it can't lose work:

- no uncommitted or untracked changes, and not locked
- no process has its cwd inside it (a running agent, shell or dev server)
- idle for `--days` (default 3)
- its HEAD is merged into the default branch, or still held by a branch. Removing a worktree
  never deletes its branch, so those commits stay reachable.

Kept worktrees say why. If they have a target dir, `--targets` can clear just that, since a
target dir is always safe to rebuild. `--main-targets` also offers target dirs in main
checkouts that have been idle for `--days`.

## 3. Report and ask

Give the user the totals and anything surprising: the biggest wins, and kept worktrees with
uncommitted changes or detached unmerged commits, since those may be abandoned work they'd
want to look at. Don't paste the whole table back unless they ask.

Then ask which clean to run. Never run `janitor clean` without an explicit yes in chat, and
pass the same options the scan used so the plan matches what they saw:

```sh
janitor clean --yes [--targets] [--main-targets] [--branches] [--days N]
```

`--branches` also deletes a removed worktree's branch, only when it's merged (`git branch -d`).
Agent branches like `worktree-agent-*` pile up otherwise.

## Don't

- Touch a `keep:` worktree by hand to "finish the job". The verdict is the safety check.
- Clear `--main-targets` in a repo where a build is running, or without saying the next build
  there will be a cold one.
