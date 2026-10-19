# shellcheck shell=sh
# Source me from any macOS-agent pipeline step:  . scripts/ci/macos-env.sh
#
# Ported from ttr3's copy, which exists because Woodpecker's LOCAL backend (the
# only backend the macOS agent has — it runs commands on the host, not in a
# container) points HOME at a throwaway per-workflow directory. That:
#
#   * breaks every keychain operation, if a pipeline ever needs signing — the
#     login-keychain path is bogus and `security list-keychains -s` writes prefs
#     nothing reads, so codesign can never resolve an identity; and
#   * defeats every cache that lives under HOME (SwiftPM, DerivedData, cargo,
#     Homebrew), because a fresh workspace is handed out each run.
#
# vision adds one thing ttr3's copy does not need: PATH. A local-backend step
# inherits the agent process's environment, and the agent is not a login shell —
# so /opt/homebrew/bin is NOT necessarily on PATH. branding/build.py shells out
# to `rsvg-convert` with `check=True` and no fallback (branding/build.py:272), so
# a missing Homebrew PATH is a hard failure whose message ("No such file or
# directory: 'rsvg-convert'") reads as a missing dependency rather than as a
# missing PATH entry. Prepending it is cheap and removes the whole class.

HOME="$(dscl . -read "/Users/$(id -un)" NFSHomeDirectory | awk '{print $2}')"
export HOME

# Apple Silicon Homebrew. /usr/local/bin covers an Intel agent if one is ever
# added; both are harmless when absent.
PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
export PATH

CACHE="$HOME/.cache/woodpecker/standards"
mkdir -p "$CACHE"
export CACHE

# Xcode: reuse one DerivedData across runs (in-workspace would be thrown away).
DERIVED_DATA="$CACHE/DerivedData"
export DERIVED_DATA
