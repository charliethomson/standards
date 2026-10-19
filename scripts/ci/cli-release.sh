#!/usr/bin/env bash
# Build the `standards` cli for one target and attach it to the registry version for
# this commit, as a DRAFT. One call per leg of .woodpecker/cli.release.yml.
#
# Adapted from vision's scripts/ci/cli-release.sh: same flow (version, build, stamp
# check, package, register, attach --no-finalize), minus vision's darwin codesigning.
# vision signs because its cli keeps a credential in the login Keychain, which trusts
# the signing identity; `standards` stores nothing in the Keychain, so an unsigned
# (linker ad-hoc signed) binary costs nothing, and the installer fetches with curl,
# which sets no quarantine bit. Revisit if the cli ever keeps a Keychain item.
#
# Usage:  scripts/ci/cli-release.sh <registry-target> <rust-target-triple>
#   e.g.  scripts/ci/cli-release.sh darwin-arm64 aarch64-apple-darwin
#
# Run from the repo root of a FULL clone (history + tags): the version is the commit
# count. The caller sets up the toolchain, CARGO_TARGET_DIR and the cache guard, which
# differ per agent; everything from the build on is identical, so it lives here once.
#
# Env: REGISTRY_URL, REGISTRY_TOKEN, REGISTRY_REPO_TOKEN (see publish.sh), and optionally
#      CARGO_TARGET_DIR. Needs cargo, git, tar, jq and timeout on PATH.
#
# Never finalizes. Flipping the version live is .woodpecker/cli.release-finalize.yml's
# job, once every target is attached, so a leg that fails leaves a draft nobody can
# install, never a live version missing a platform.
#
# On a pull_request (CI_PIPELINE_EVENT) it builds, checks the version stamp and packages
# the archive, then stops before the registry. The registry env is not needed on that
# path, and the PR steps do not pass it.
set -euo pipefail

target="${1:?cli-release.sh needs a registry target, e.g. darwin-arm64}"
triple="${2:?cli-release.sh needs a rust target triple, e.g. aarch64-apple-darwin}"

# L3: registry product dev.thmsn.standards, app cli; crate standards-cli, bin standards.
# The cli's self-update resolves the same pair (crates/standards-cli/src/self_update.rs).
PRODUCT=dev.thmsn.standards
APP=cli
BIN_NAME=standards
INSTALL_MANIFEST=crates/standards-cli/packaging/install.json

# The canonical version (docs/versioning.md). Every leg derives it from the same commit,
# so they agree without passing anything between agents; they must, or they would attach
# to two different drafts and neither would ever finalize.
eval "$(sh scripts/ci/version.sh)"
# standards-core's build.rs bakes RELEASE_VERSION into `standards --version`, so the
# binary and the registry version are one value.
export RELEASE_VERSION
echo "cli-release: standards $RELEASE_VERSION (build $BUILD) for $target ($triple)"

# --locked: a release builds exactly the committed Cargo.lock.
cargo build --release --locked -p standards-cli --bin "$BIN_NAME" --target "$triple"

bin="${CARGO_TARGET_DIR:-target}/$triple/release/$BIN_NAME"
[ -x "$bin" ] || { echo "cli-release: no binary at $bin" >&2; exit 1; }

# Prove the stamp before publishing it. A mismatch would ship a binary that reports a
# different version than the registry lists; the installer's up-to-date check and
# `standards self-update` both compare against it, so it would reinstall forever. Both
# legs build natively, so the binary always runs on the agent that built it.
reported="$("$bin" --version)"
if [ "$reported" != "$BIN_NAME $RELEASE_VERSION" ]; then
  echo "cli-release: binary reports '$reported', expected '$BIN_NAME $RELEASE_VERSION'" >&2
  exit 1
fi

# The archive holds the bare binary at its root: self-update takes the entry named
# `standards` out of the .tar.gz. Nothing else ships (no completions subcommand yet).
rm -rf dist && mkdir -p dist/stage
cp "$bin" "dist/stage/$BIN_NAME"
archive="dist/$BIN_NAME-$target.tar.gz"
tar -czf "$archive" -C dist/stage "$BIN_NAME"

if [ "${CI_PIPELINE_EVENT:-}" = "pull_request" ]; then
  echo "cli-release: pull_request: built and packaged $archive ($(wc -c <"$archive" | tr -d ' ') bytes); not publishing"
  exit 0
fi

# ---- registry metadata --------------------------------------------------------
#
# ensure-product / ensure-app are idempotent: a no-op once the entity exists (metadata
# is left alone without --update), so a fresh registry needs no manual step. No icon:
# this repo has no branding (AGENTS.md, "This repo's own code").
#
# Retried once because the legs run concurrently: on the very first release both can
# see "not found" and both POST, and the loser's create fails with a conflict. By the
# retry the entity exists, so the second attempt is the no-op path.
register() {
  bash scripts/ci/publish.sh ensure-product --product "$PRODUCT" \
    --name standards --developer "Charlie Thomson" \
    --description "The fleet's engineering standards, served over HTML, REST and MCP" \
    --homepage https://standards.thmsn.dev \
    --repo https://github.com/charliethomson/standards &&
    bash scripts/ci/publish.sh ensure-app --product "$PRODUCT" --app "$APP" \
      --kind cli --name "standards cli" --bin-name "$BIN_NAME" \
      --install "$INSTALL_MANIFEST"
}
register || {
  echo "cli-release: registering $PRODUCT/$APP failed; retrying once (the other leg may have just created it)" >&2
  sleep 5
  register
}

# ---- attach -------------------------------------------------------------------
#
# --install and --notes on EVERY leg: publish-cli sends them only with the draft
# *create*, and whichever leg gets there first creates it; a leg arriving second gets a
# 409 and only attaches. With them on one leg only, about half the releases would ship
# without the manifest (ttr3 learned this the hard way).
#
# --skip-existing so re-running a leg for the same commit is a no-op rather than a 409
# on the artifact it already attached.
#
# Capped at 15 minutes: the publish takes seconds, and a stall must cost a legible
# failure rather than the whole workflow deadline.
notes="$(git log -1 --pretty=format:%B)"
[ -n "$notes" ] || notes="standards $RELEASE_VERSION"

timeout 900 bash scripts/ci/publish.sh publish-cli \
  --product "$PRODUCT" --app "$APP" \
  --version "$RELEASE_VERSION" --build "$BUILD" --notes "$notes" \
  --bin-name "$BIN_NAME" --install "$INSTALL_MANIFEST" \
  --artifact "$target=$archive" \
  --no-finalize --skip-existing || {
  st=$?
  if [ "$st" -eq 124 ]; then
    echo "cli-release: publish-cli BLOCKED, killed at the 900s cap" >&2
  else
    echo "cli-release: publish-cli exited $st" >&2
  fi
  exit "$st"
}

echo "cli-release: attached $target to $PRODUCT/$APP $RELEASE_VERSION (draft until cli.release-finalize)"
