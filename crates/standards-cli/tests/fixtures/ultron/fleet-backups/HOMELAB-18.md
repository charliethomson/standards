# HOMELAB-18 — game box mounts /mnt/truenas/Secure + nas-guard

PLAN
- [x] 1. vision start comment with this plan
- [x] 2. confirm nas-guard/installer need no change for game (HOMELAB-17 made fstab-absent mounts skip) — re-run harness game case
- [x] 3. docs: media/README guarded-mounts table (+game) and a "Setting up a new host" subsection with the mount + install steps; CLAUDE.md "State that isn't in this repo" row for game (fstab line + nas-guard); docs/HOSTS.md game row; docs/RUNBOOK.md remote-host section
- [x] 4. commit (explicit paths), vision finish comment with human steps

STATE
- agent part done; finishing comment posted. Human part pending (user).

TOUCHED
- commit 22a49bf (homelab main, not pushed): media/README.md, CLAUDE.md, docs/RUNBOOK.md, docs/HOSTS.md

VERIFY
- shellcheck media/host/*.sh ; bash <scratchpad>/nasguard-test/run.sh (game case)
- human: findmnt /mnt/truenas/Secure on game; systemctl is-active nas-guard.timer; test container restarted after remount
- last result: shellcheck clean; harness game case ok
