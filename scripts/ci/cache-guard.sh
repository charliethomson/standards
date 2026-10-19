#!/usr/bin/env sh
# Keep a build cache from filling the agent's disk.
#
# Usage:  scripts/ci/cache-guard.sh <dir> <max-gb> [min-free-gb]
#
# Wipes <dir> when it exceeds <max-gb> OR when the filesystem holding it has
# less than [min-free-gb] free (default 12) — because the cache being small
# doesn't help if something else filled the disk, and a build that starts with
# too little headroom dies with "No space left on device" partway through.
#
# Why this exists — two incidents, same class:
#   * ttr3: an unbounded cargo target dir (they never shrink) took agent-2 to
#     100% of a 76G disk. A full disk truncates apt's downloads, which surfaces
#     as "GPG error ... At least one invalid signature was encountered" on every
#     build — a symptom that points nowhere near the actual cause.
#   * vision, 2026-08-14: 22 stale agent worktrees on the dev Mac, 54G, and no
#     build was possible until they were cleared. Different mechanism, same
#     failure: nothing was bounding growth.
#
# Copied from vision's scripts/ci/cache-guard.sh unchanged but for this note.
#
# Like argus's and vision's copies, standards persists its cargo target dirs as bind-mounted
# volumes, so <dir> is usually a MOUNT POINT. `rm -rf` on a mount point fails
# with "Device or resource busy" — the directory itself can't go, only what's
# inside it. So the wipe clears contents instead.
#
# On sizing: do not copy another repo's numbers. ttr3 uses 9G/14G for a far
# larger workspace (BoringSSL, gpui); argus deliberately sized DOWN to 4G/5G
# because its agent reported only 10G free. standards' callers pick their own —
# see the per-step comments in .woodpecker/{rust.ci,cli.release}.yml.
#
# Cargo's own caches (<dir> under $CARGO_HOME, e.g. $CARGO_HOME/registry) are shared by
# every Linux step on the agent through one CARGO_HOME host dir (STD-33), so another
# container may be unpacking into <dir> or compiling from it right now. The wipe then
# takes cargo's MutateExclusive lock first, the way cargo itself does before deleting
# cache contents: flock on $CARGO_HOME/.package-cache-mutate, THEN on
# $CARGO_HOME/.package-cache (cargo's order; the reverse can deadlock against a cargo
# that holds the shared mutate lock and wants the download lock). It blocks until every
# other cargo on the agent has finished its build. The size check itself takes no lock,
# so a guard that wipes nothing never waits.
set -eu

dir="${1:?cache-guard.sh needs a directory}"
max="${2:?cache-guard.sh needs a size cap in GB}"
min_free="${3:-12}"

mkdir -p "$dir"

# Empty <dir> without removing it. Handles dotfiles, and an empty dir is a
# no-op rather than an error.
wipe_contents() {
  find "$dir" -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null || true
}

# Under $CARGO_HOME (and not CARGO_HOME itself): hold cargo's locks for the wipe.
wipe() {
  case "${CARGO_HOME:-}" in
    "") wipe_contents; return ;;
  esac
  case "$dir" in
    "$CARGO_HOME"/?*) ;;
    *) wipe_contents; return ;;
  esac
  command -v flock >/dev/null || {
    echo "cache-guard: flock not found; refusing to wipe $dir under a shared CARGO_HOME" >&2
    return 1
  }
  echo "cache-guard: waiting for cargo's cache locks in $CARGO_HOME"
  (
    exec 8>"$CARGO_HOME/.package-cache-mutate" 9>"$CARGO_HOME/.package-cache"
    flock -x 8
    flock -x 9
    wipe_contents
  )
}

used_gb="$(du -sk "$dir" 2>/dev/null | awk '{print int($1/1048576)}')"
free_gb="$(df -Pk "$dir" 2>/dev/null | awk 'NR==2 {print int($4/1048576)}')"
used_gb="${used_gb:-0}"
free_gb="${free_gb:-0}"

if [ "$used_gb" -ge "$max" ]; then
  echo "cache-guard: $dir is ${used_gb}G (cap ${max}G) — wiping"
  wipe
elif [ "$free_gb" -lt "$min_free" ]; then
  # Worth saying plainly: if the cache is already near 0G, wiping it frees
  # nothing and the disk pressure is coming from elsewhere on the agent.
  echo "cache-guard: only ${free_gb}G free (want ${min_free}G) — wiping $dir (${used_gb}G)"
  wipe
else
  echo "cache-guard: $dir ${used_gb}G / ${max}G, ${free_gb}G free"
fi

# Report what's left so a disk problem is visible in the log next time.
df -h "$dir" | tail -1
