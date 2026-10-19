#!/usr/bin/env bash
# ── standards: a COPY of vision's copy of ttr3's script. Read before editing. ───
# Copied verbatim from vision's scripts/ci/publish.sh (2026-10-19) on the program's
# locked decision L20. docs/registry-publishing.md says not to copy this script into
# another repo and to port the release pipeline to kiln (libci::registry) instead;
# that port is the follow-up, and this copy is the known divergence until then.
# Everything below this block is vision's file unchanged.
#
# ── vision: a COPY of ttr3's script. Read before editing. ──────────────────────
# Copied verbatim from ttr3's scripts/ci/publish.sh (ttr3 34714d4, 2026-09-08);
# nothing below this block differs. It is shared tooling, so a fix belongs in
# ttr3 first and is copied here, not diverged. Nothing needed parameterising:
# the product, app and version all arrive as arguments from the caller.
#
# This is the publish path the standards prescribe for a Woodpecker repo: the
# registry's composite action is GitHub-only, so a release pipeline calls the
# registry from a scripts/ci/publish.sh (standards/docs/registry-publishing.md).
#
# The comments below are ttr3's and describe ttr3's pipelines — the Windows leg,
# the darwin leg's stall, macos-keychain.sh — none of which vision has.
# ──────────────────────────────────────────────────────────────────────────────
#
# Publish an artifact to the release registry.
#
# The registry's publish script lives in the private charliethomson/registry
# repo, so every release pipeline was cloning it and invoking it by hand — five
# copies of the same six lines. This is that, once.
#
# Usage:  scripts/ci/publish.sh <subcommand> [args...]
#   e.g.  scripts/ci/publish.sh publish --product dev.thmsn.ttr --app macos \
#             --version "$VERSION" --file "$DMG" --build "$BUILD" --notes "$NOTES"
#
# Env: REGISTRY_URL, REGISTRY_TOKEN (read by the registry script itself) and
#      REGISTRY_REPO_TOKEN (read-only PAT used to fetch the script).
set -euo pipefail

: "${REGISTRY_REPO_TOKEN:?publish.sh needs REGISTRY_REPO_TOKEN}"
: "${REGISTRY_TOKEN:?publish.sh needs REGISTRY_TOKEN}"
: "${REGISTRY_URL:?publish.sh needs REGISTRY_URL}"

dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT

# Progress markers to stderr, unbuffered. The caller's `set -x` stops at this
# script's boundary, and a step killed mid-run loses whatever is still sitting
# in a block-buffered stdout — which is one reason the darwin leg's log simply
# stopped. These name the phase so a kill still says how far it got. Values are
# never echoed; the registry token lives in the URL below and in the child's
# environment, and neither is printed.
phase() { echo "[publish.sh $(date +%H:%M:%S)] $*" >&2; }
phase "start: $1"

# GIT_TERMINAL_PROMPT=0 is load-bearing, not tidiness. With a *blank* token the
# URL becomes `https://x-access-token:@github.com/...`, and git treats a missing
# password as "ask the user" — so on an agent with no TTY it blocks forever
# rather than failing. That is how `cli.release > darwin` burned the entire
# 60-minute workflow deadline and reached the registry with nothing: the step
# was sitting on a credential prompt nobody could answer. A bad-but-present
# token 401s immediately; only an empty one hangs, which is exactly the case
# a CI box hits when secret injection silently gives it nothing.
#
# stderr used to go to /dev/null, which is why the log showed nothing at all.
# Keep it: a failure here needs to be legible, and the token is in the URL, not
# in git's error output.
#
# No credential helper: the token is in the URL, so the helper has nothing to
# look up, and its only remaining move is to *store* the token after the clone
# succeeds. On the mac agent that store went through osxkeychain into a search
# list wrecked by killed signing runs (see macos-keychain.sh) and took ~135s —
# past this timeout, twice in a row, for a clone that otherwise takes half a
# second. The publish must not depend on the state of the host keychain at all.
phase "cloning the registry repo"
GIT_TERMINAL_PROMPT=0 timeout 120 git -c credential.helper= clone --depth 1 \
  "https://x-access-token:$REGISTRY_REPO_TOKEN@github.com/charliethomson/registry.git" \
  "$dir/registry" || {
  status=$?
  if [ "$status" -eq 124 ]; then
    echo "publish.sh: cloning the registry repo timed out after 120s" >&2
  else
    echo "publish.sh: cloning the registry repo failed (exit $status)" >&2
  fi
  exit "$status"
}

# Release notes come in through the environment rather than as an argument.
#
# They're arbitrary commit text — spaces, newlines, quotes — and the Windows
# agent invokes this script through PowerShell, whose native-argument quoting
# does not survive that. A body containing "go and search" reached the registry
# script as separate argv entries and it died with `unknown arg go`, *after*
# building and uploading nothing. Passing it via the environment keeps the value
# intact across the PowerShell → bash boundary; quoting it here is reliable.
#
# Callers that already pass `--notes` (the bash-only macOS pipelines) are
# unaffected: this only appends when the variable is set.
args=("$@")
case "${1:-}" in
  publish | publish-cli)
    # Only these take --notes. Appending it unconditionally made a later
    # `publish.sh finalize` in the same job die with "unknown arg --notes",
    # because RELEASE_NOTES was still set from the publish call before it.
    if [ -n "${RELEASE_NOTES:-}" ]; then
      args+=(--notes "$RELEASE_NOTES")
    fi
    ;;
esac

phase "registry repo cloned; handing off to registry-publish.sh ${1:-}"
bash "$dir/registry/scripts/registry-publish.sh" "${args[@]}"
phase "registry-publish.sh returned $?"
