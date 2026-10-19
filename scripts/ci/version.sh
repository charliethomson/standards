#!/usr/bin/env sh
# Canonical fleet version (docs/versioning.md): <tag MAJOR.MINOR>.<commit count>.
# Copied from vision's scripts/ci/version.sh; only the comments differ.
#
# The image build needs this because .dockerignore keeps `.git` out of the Docker
# context, so build.rs can't derive the version itself and takes RELEASE_VERSION as
# a build-arg instead. The cli release legs use it so every leg agrees.
#
# Usage:  eval "$(scripts/ci/version.sh)"   # -> sets RELEASE_VERSION, VERSION and BUILD
#
# RELEASE_VERSION is the name docs/versioning.md and every build.rs use; VERSION is
# the same value under the name docker-build.sh still reads.
#
# The tag rule is mirrored for local builds in crates/standards-core/build.rs, which
# uses `git describe` (current branch only); tags go on main, so the two agree.
set -eu

# plugin-git's clone doesn't reliably populate local tag refs, so refetch.
git fetch --tags --quiet origin 2>/dev/null || true

COUNT="$(git rev-list --count HEAD)"

# Read MAJOR.MINOR from release tags: exactly vMAJOR.MINOR or vMAJOR.MINOR.PATCH,
# anchored at both ends, so a platform tag like macos-v0.1.318 (or a v1.2-rc1)
# never sets the version (VERSIONING.md, "Tag convention"). Filtered in-shell rather
# than with `git tag -l 'v[0-9]*'`, whose glob comes back empty in CI. This is
# also why it is not `git describe --tags --match 'v[0-9]*'`, which is what the
# GitHub workflows used: describe walks only the current branch's ancestry, so a
# tag on a sibling branch is invisible to it.
#
# Sorted numerically FIELD BY FIELD, not lexically: a plain `sort` puts 0.9 after
# 0.10, so the first v0.10.x tag would drag every pipeline back to 0.9.<count> —
# below what is already live, which reads to clients as "no update available"
# forever. `sort -V` would do it but BSD sort has no -V, hence explicit numeric
# keys (portable across GNU and BSD — this runs on the Linux agents AND on the
# macOS one).
MM="$(git tag -l | sed -nE 's/^v([0-9]+\.[0-9]+)(\.[0-9]+)?$/\1/p' | sort -t. -k1,1n -k2,2n | tail -1)"

# No tag at all is legitimate exactly once, before the first release tag (v0.1 at
# go-live). Every other time it means the fetch above
# failed, and falling back silently would publish below what is live. Cheap to
# make loud; expensive to debug from a version number that looks plausible.
if [ -z "$MM" ]; then
  echo "version.sh: no vMAJOR.MINOR tag found, falling back to 0.0" >&2
  echo "version.sh:   expected while standards is untagged; once it HAS release tags" >&2
  echo "version.sh:   this means the fetch above failed." >&2
  MM="0.0"
fi

printf 'RELEASE_VERSION=%s\nVERSION=%s\nBUILD=%s\n' "$MM.$COUNT" "$MM.$COUNT" "$COUNT"
