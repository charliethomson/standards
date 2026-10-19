#!/usr/bin/env sh
# Dependency audit for .woodpecker/audit.ci.yml (standards/docs/ci-cd.md,
# "Dependency audit"): collect the reports, then gate on critical only.
#
# Usage:
#   scripts/ci/audit.sh cargo <workspace-dir> <name>   # server cargo-server
#   scripts/ci/audit.sh npm   <package-dir>   <name>   # apps/web npm-web
#   scripts/ci/audit.sh gate  <name>...                # cargo-server cargo-cli npm-web
#
# The collect commands print the tool's own report to the log (that is the
# visibility half) and save it under .audit/ in the workspace, which Woodpecker
# steps share. They exit 0 when the tool produced a report, whatever it found,
# and non-zero only when it produced none (a failed advisory-db fetch, an npm
# registry error). `gate` then reads the saved reports, so it needs neither
# toolchain, only jq.
#
# Why not trust the audit tools' exit codes: both exit 1 for "found something"
# AND for "could not audit at all" (cargo-audit 0.21.2 did exactly that on the
# CVSS 4.0 advisories, per ci-cd.md). So `gate` fails when a report it was
# told to expect is missing: nothing audited is a red gate, never a green one.
#
# Severity:
#   * npm reports it per advisory in `npm audit --json`.
#   * cargo-audit's JSON carries only the CVSS vector; the score is computed
#     by cargo-audit and printed in its TEXT report ("Severity:  9.8 (critical)"),
#     which handles CVSS 3.x and 4.0 alike. So `cargo` saves both and `gate`
#     joins them by advisory id. An advisory that HAS a vector but no severity
#     in the text fails the gate: that means the text format moved, and guessing
#     would be silence. An advisory with no vector at all is "unscored" (RustSec
#     leaves some unscored) and reported, not gated.
#
# Exceptions live in scripts/ci/audit-exceptions.txt, one per line:
#   <advisory id>  <expires YYYY-MM-DD>  <rationale…>
# An exception is honoured through its expiry date and fails the gate the day
# after, whether or not the advisory still applies, so the file cannot quietly
# fill with dead entries. A line missing a date or a rationale fails too.
set -eu

# Overridable for testing the gate against fixture reports.
out="${AUDIT_OUT:-.audit}"
exceptions="${AUDIT_EXCEPTIONS:-$(dirname "$0")/audit-exceptions.txt}"

usage() {
  echo "usage: audit.sh cargo <dir> <name> | npm <dir> <name> | gate <name>..." >&2
  exit 2
}

cmd="${1:-}"
[ -n "$cmd" ] || usage
shift

case "$cmd" in
  cargo)
    dir="${1:?audit.sh cargo needs a workspace dir}"
    name="${2:?audit.sh cargo needs a report name}"
    mkdir -p "$out"
    rm -f "$out/$name.json" "$out/$name.txt"
    # --db outside CARGO_HOME so the pipeline can keep it in a cache volume.
    db="${AUDIT_DB:-${CARGO_HOME:-$HOME/.cargo}/advisory-db}"
    echo "audit: cargo audit $dir (db $db)"
    # Text first, fetching the db; its output is the human report.
    (cd "$dir" && cargo audit --color never --db "$db" --file Cargo.lock) >"$out/$name.txt" 2>&1 || true
    cat "$out/$name.txt"
    (cd "$dir" && cargo audit --db "$db" --no-fetch --file Cargo.lock --json) >"$out/$name.json" 2>/dev/null || true
    if ! grep -q '"vulnerabilities"' "$out/$name.json" || ! grep -q 'Scanning' "$out/$name.txt"; then
      echo "audit: cargo audit produced no report for $dir — nothing was audited" >&2
      rm -f "$out/$name.json" "$out/$name.txt"
      exit 1
    fi
    ;;

  npm)
    dir="${1:?audit.sh npm needs a package dir}"
    name="${2:?audit.sh npm needs a report name}"
    mkdir -p "$out"
    rm -f "$out/$name.json"
    echo "audit: npm audit $dir"
    # --package-lock-only: audit the committed lockfile, no node_modules needed.
    (cd "$dir" && npm audit --package-lock-only) || true
    (cd "$dir" && npm audit --package-lock-only --json) >"$out/$name.json" 2>/dev/null || true
    if ! grep -q '"auditReportVersion"' "$out/$name.json"; then
      echo "audit: npm audit produced no report for $dir — nothing was audited" >&2
      cat "$out/$name.json" >&2 || true
      rm -f "$out/$name.json"
      exit 1
    fi
    ;;

  gate)
    [ "$#" -gt 0 ] || usage
    command -v jq >/dev/null || { echo "audit: gate needs jq" >&2; exit 2; }
    today="$(date -u +%Y-%m-%d)"
    fail=0
    findings="$(mktemp)"
    trap 'rm -f "$findings"' EXIT

    # ---- exceptions ----------------------------------------------------------
    # Emits "id<TAB>expires" for each valid, unexpired entry on stdout; prints
    # and counts every problem on stderr (the count is the exit status).
    valid_exceptions=""
    if [ -f "$exceptions" ]; then
      valid_exceptions="$(awk -v today="$today" '
        /^[[:space:]]*(#|$)/ { next }
        {
          id = $1; expires = $2
          rationale = $0
          sub(/^[[:space:]]*[^[:space:]]+[[:space:]]+[^[:space:]]+[[:space:]]*/, "", rationale)
          where = FILENAME ":" FNR
          if (id !~ /^(RUSTSEC-[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9]|GHSA-[0-9a-z][0-9a-z][0-9a-z][0-9a-z]-[0-9a-z][0-9a-z][0-9a-z][0-9a-z]-[0-9a-z][0-9a-z][0-9a-z][0-9a-z])$/) {
            printf "audit: %s: %s is not a RUSTSEC-/GHSA- advisory id\n", where, id > "/dev/stderr"; bad++; next
          }
          if (expires !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) {
            printf "audit: %s: %s has no expiry date (YYYY-MM-DD); exceptions must expire\n", where, id > "/dev/stderr"; bad++; next
          }
          if (rationale == "") {
            printf "audit: %s: %s has no rationale\n", where, id > "/dev/stderr"; bad++; next
          }
          if (expires < today) {
            printf "audit: %s: exception for %s EXPIRED on %s — re-triage it: upgrade, or renew with a new date and reason\n", where, id, expires > "/dev/stderr"; bad++; next
          }
          printf "%s\t%s\n", id, expires
        }
        END { exit (bad > 0) }
      ' "$exceptions")" || fail=1
    fi

    # ---- findings: severity<TAB>id<TAB>report<TAB>package<TAB>title ------------
    for name in "$@"; do
      case "$name" in
        cargo-*)
          json="$out/$name.json"; txt="$out/$name.txt"
          if [ ! -s "$json" ] || [ ! -s "$txt" ]; then
            echo "audit: no $name report — its collect step failed, so nothing was audited" >&2
            fail=1; continue
          fi
          # id -> severity label, from cargo-audit's text blocks. Empty fields are
          # "-" throughout: tab is IFS whitespace, so `read` would collapse an
          # empty one and shift every field after it.
          # ANSI escapes are stripped first: cargo colours its output whenever the
          # agent's environment forces colour, and a coloured "ID:" never matches.
          esc="$(printf '\033')"
          sev_map="$(sed "s/${esc}\[[0-9;]*m//g" "$txt" | awk '
            /^ID:/       { id = $2 }
            /^Severity:/ { s = $0; sub(/.*\(/, "", s); sub(/\).*/, "", s); print id "\t" s }
          ')"
          jq -r '.vulnerabilities.list[] |
                 [.advisory.id, (.advisory.cvss // "-"), "\(.package.name)@\(.package.version)", (.advisory.title // "-")] | @tsv' "$json" |
            while IFS="$(printf '\t')" read -r id cvss pkg title; do
              sev="$(printf '%s\n' "$sev_map" | awk -F '\t' -v id="$id" '$1 == id { print $2; exit }')"
              if [ -z "$sev" ]; then
                if [ "$cvss" != "-" ]; then sev="UNKNOWN"; else sev="unscored"; fi
              fi
              printf '%s\t%s\t%s\t%s\t%s\n' "$sev" "$id" "$name" "$pkg" "$title"
            done >>"$findings"
          ;;
        npm-*)
          json="$out/$name.json"
          if [ ! -s "$json" ]; then
            echo "audit: no $name report — its collect step failed, so nothing was audited" >&2
            fail=1; continue
          fi
          jq -r '[.vulnerabilities[] | .via[] | objects |
                  [.severity, ((.url // "-") | split("/") | last), .name, (.title // "-")]] | unique[] |
                 [.[0], .[1], "'"$name"'", .[2], .[3]] | @tsv' "$json" >>"$findings"
          ;;
        *) echo "audit: unknown report $name (want cargo-* or npm-*)" >&2; fail=1 ;;
      esac
    done

    # ---- verdict -------------------------------------------------------------
    total="$(wc -l <"$findings" | tr -d ' ')"
    echo "audit: $total advisories across: $*"
    blocking=0
    while IFS="$(printf '\t')" read -r sev id report pkg title; do
      until_date="$(printf '%s\n' "$valid_exceptions" | awk -F '\t' -v id="$id" '$1 == id { print $2; exit }')"
      status=""
      case "$sev" in
        critical)
          if [ -n "$until_date" ]; then status="excepted until $until_date"
          else status="BLOCKING"; blocking=$((blocking + 1)); fi ;;
        UNKNOWN)
          status="BLOCKING (has a CVSS vector but no severity in cargo-audit's report)"
          blocking=$((blocking + 1)) ;;
      esac
      printf '  %-9s %-20s %-13s %-28s %s%s\n' "$sev" "$id" "$report" "$pkg" "$title" "${status:+  [$status]}"
    done <"$findings"

    if [ "$blocking" -gt 0 ]; then
      echo "audit: $blocking blocking advisory(ies). Triage within 24h: upgrade, mitigate, or add an" >&2
      echo "audit: expiring exception to scripts/ci/audit-exceptions.txt." >&2
      fail=1
    fi
    [ "$fail" -eq 0 ] && echo "audit: gate passed (nothing critical outside a live exception)"
    exit "$fail"
    ;;

  *) usage ;;
esac
