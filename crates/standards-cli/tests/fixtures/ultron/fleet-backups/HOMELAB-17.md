# HOMELAB-17 — nas-guard covers /mnt/truenas/Secure

PLAN
- [x] 1. vision start comment with this plan
- [x] 2. nas-guard.sh: mount list as data (NAS_GUARD_MOUNTS, default Files:Secure), unit derived via systemd-escape, mounts absent from fstab skipped (so the same script runs on game), per-mount retry, sweep binds under any mounted guarded mount; prefix match fixed to m or m/
- [x] 3. install-nas-guard.sh: comments generic, warn if no guarded mount is in fstab
- [x] 4. media/README.md nas-guard section + docs/RUNBOOK.md (prereqs + dev-box DR) + CLAUDE.md row wording
- [x] 5. local test: shellcheck; dry-run harness with fake findmnt/docker/systemctl/systemd-escape and fake /proc mountinfo (NAS_GUARD_PROC override)
- [x] 6. commit (explicit paths), vision finish comment

STATE
- agent part done; finishing comment posted. Human part pending (user).

TOUCHED
- commit e2fe997 (homelab main, not pushed): media/host/nas-guard.sh, media/host/install-nas-guard.sh, media/README.md, docs/RUNBOOK.md, CLAUDE.md

VERIFY
- shellcheck media/host/nas-guard.sh media/host/install-nas-guard.sh
- bash <scratchpad>/nasguard-test/run.sh
- last result: shellcheck clean (both scripts); harness 12/12 ok (scratchpad nasguard-test/run.sh, not committed)
