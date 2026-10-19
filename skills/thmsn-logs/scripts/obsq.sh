#!/usr/bin/env bash
# obsq — query a product's logs, metrics and traces in the homelab Grafana
# (Loki / Prometheus / Tempo) through the datasource proxy, with curl + jq.
# See ../SKILL.md for the cookbook. Requires: curl, jq, GNU date (gdate on macOS).
#
# Usage:
#   obsq.sh logs    '<logql>'  [since] [until] [limit]   # log lines: time [LEVEL] [scope] body
#   obsq.sh meta    '<logql>'  [since] [until] [limit]   # full structured metadata of matches
#   obsq.sh top     '<logql>'  [since] [until] [limit]   # matches grouped by level+scope+body, commonest first
#   obsq.sh series  '<logql>'  [at]                      # instant LogQL aggregation -> labels = value
#   obsq.sh prom    '<promql>' [at]                      # instant PromQL -> labels = value
#   obsq.sh trace   <trace-id>                           # one trace from Tempo -> span lines
#   obsq.sh ref     <hex prefix> [since] [until]         # a client's `Ref:` -> full trace id(s)
#   obsq.sh run     <recipe> [--range 7d] [args...]      # a named query (see `recipes`)
#   obsq.sh recipes                                      # list recipes: the repo's, then the fleet's
#   obsq.sh env                                          # print the resolved config
#   obsq.sh init                                         # scaffold this repo's .standards.d/thmsn-logs/
#   obsq.sh <command> [args...]                          # a repo command from .standards.d/thmsn-logs/commands/
#
# In any query, {{sel}} expands to the repo's stream selector and {{svc}} to its
# first service, so `obsq.sh logs '{{sel}} | severity_text=`ERROR`'` works in every repo.
#
# since/until/at are GNU-date exprs ('30 minutes ago', '2026-06-10T05:00:00Z') or
# epoch seconds. Defaults: since='1 hour ago', until=now, limit=100, at=now.
#
# Per-repo specialisation lives in <repo>/.standards.d/thmsn-logs/ (see ../SKILL.md):
#   config      KEY=VALUE, sourced: SERVICES, SELECTOR, GRAFANA_URL, token source, UIDs
#   queries/    <name>.logql | <name>.promql recipes; a repo recipe shadows a fleet one
#   commands/   executables that become extra subcommands, run with the resolved env
#   NOTES.md    the repo's fields, gotchas and cookbook (read by the agent, not by this script)
set -euo pipefail

SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${OBSQ_REPO:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
OVERLAY="$REPO_ROOT/.standards.d/thmsn-logs"

die() { echo "obsq: $*" >&2; exit 1; }

# --- config: fleet defaults, then .standards.conf, then the repo's overlay ----------
GRAFANA_URL="https://grafana.dev.thmsn.dev"
GRAFANA_ENV_FILE=""   # set in the overlay to pin one; otherwise the standards mcp/.env files below
GRAFANA_TOKEN_VAR="GRAFANA_SERVICE_ACCOUNT_TOKEN"
LOKI_UID="loki"; PROM_UID="prometheus"; TEMPO_UID="tempo"
SERVICES=""; SELECTOR=""; PRODUCT=""
# shellcheck disable=SC1091
[ -f "$REPO_ROOT/.standards.conf" ] && . "$REPO_ROOT/.standards.conf"
# shellcheck disable=SC1091
[ -f "$OVERLAY/config" ] && . "$OVERLAY/config"

# Every service named in SERVICES, or every dev.thmsn.<product>.* service.
if [ -z "$SELECTOR" ]; then
  if [ -n "$SERVICES" ]; then
    # shellcheck disable=SC2086
    alt="$(printf '%s|' $SERVICES)"; alt="${alt%|}"
    case "$alt" in *'|'*) SELECTOR="{service_name=~\"$alt\"}" ;; *) SELECTOR="{service_name=\"$alt\"}" ;; esac
  elif [ -n "$PRODUCT" ]; then
    SELECTOR="{service_name=~\"dev.thmsn.$PRODUCT.*\"}"
  fi
fi
FIRST_SERVICE="${SERVICES%% *}"; FIRST_SERVICE="${FIRST_SERVICE:-${PRODUCT:+dev.thmsn.$PRODUCT.server}}"

# The read-only service-account token: the env var, else the standards checkout's
# gitignored mcp/.env (standards/mcp/README.md), the one this skill came from first,
# then a sibling ~/git/standards. The first file that has the key wins.
TOKEN="${!GRAFANA_TOKEN_VAR:-}"
TOKEN_FROM="\$$GRAFANA_TOKEN_VAR"
if [ -z "$TOKEN" ]; then
  for f in ${GRAFANA_ENV_FILE:+"$GRAFANA_ENV_FILE"} "$SKILL_DIR/../../mcp/.env" \
           "$REPO_ROOT/standards/mcp/.env" "$HOME/git/standards/mcp/.env"; do
    [ -f "$f" ] || continue
    TOKEN="$(grep -E "^$GRAFANA_TOKEN_VAR=" "$f" | cut -d= -f2- || true)"
    [ -n "$TOKEN" ] && { TOKEN_FROM="$f"; break; }
  done
fi

LOKI="$GRAFANA_URL/api/datasources/proxy/uid/$LOKI_UID/loki/api/v1"
PROM="$GRAFANA_URL/api/datasources/proxy/uid/$PROM_UID/api/v1"
TEMPO="$GRAFANA_URL/api/datasources/proxy/uid/$TEMPO_UID/api"
export OBSQ_REPO="$REPO_ROOT" OBSQ_SELECTOR="$SELECTOR" OBSQ_SERVICE="$FIRST_SERVICE" \
       OBSQ_LOKI="$LOKI" OBSQ_PROM="$PROM" OBSQ_TEMPO="$TEMPO" OBSQ_SCRIPT="${BASH_SOURCE[0]}"

need_token() { [ -n "$TOKEN" ] || die "no Grafana token: set $GRAFANA_TOKEN_VAR, or put it in standards/mcp/.env (cp standards/mcp/.env.example standards/mcp/.env)"; }
need_selector() { [ -n "$SELECTOR" ] || die "no selector: set SERVICES in $OVERLAY/config (obsq.sh init) or PRODUCT in .standards.conf"; }

# --- helpers ------------------------------------------------------------------------
DATE=date; command -v gdate >/dev/null 2>&1 && DATE=gdate
"$DATE" -u -d @0 +%s >/dev/null 2>&1 || die "needs GNU date (brew install coreutils)"

# A time expr to epoch seconds: bare epoch, 'now', or any GNU-date string.
secs() { case "$1" in ''|now) "$DATE" +%s ;; *[!0-9]*) "$DATE" -u -d "$1" +%s ;; *) echo "$1" ;; esac; }
hms() { "$DATE" -u -d "@$1" +%m-%dT%H:%M:%S; }

# {{sel}} / {{svc}} / {{range}} / {{1}}..{{9}} substitution.
expand() {
  local s="$1"; shift
  s="${s//\{\{sel\}\}/$SELECTOR}"; s="${s//\{\{svc\}\}/$FIRST_SERVICE}"; s="${s//\{\{range\}\}/${RANGE:-1h}}"
  local i=1; for a in "$@"; do s="${s//\{\{$i\}\}/$a}"; i=$((i + 1)); done
  case "$s" in *'{{sel}}'*|*'{{svc}}'*) need_selector ;; esac
  printf '%s' "$s"
}

loki_range() { # $1=query $2=start_ns $3=end_ns $4=limit
  need_token
  curl -sf -G -H "Authorization: Bearer $TOKEN" "$LOKI/query_range" \
    --data-urlencode "query=$1" --data-urlencode "start=$2" --data-urlencode "end=$3" \
    --data-urlencode "limit=${4:-100}" --data-urlencode "direction=backward" \
    || die "Loki query failed: $1"
}
instant() { # $1=base $2=query $3=time param value
  need_token
  curl -sf -G -H "Authorization: Bearer $TOKEN" "$1/query" \
    --data-urlencode "query=$2" --data-urlencode "time=$3" || die "query failed: $2"
}

# Recipe lookup: the repo's queries/ first, then the fleet's.
recipe_file() {
  local d f
  for d in "$OVERLAY/queries" "$SKILL_DIR/queries"; do
    for f in "$d/$1.logql" "$d/$1.promql"; do [ -f "$f" ] && { echo "$f"; return; }; done
  done
  return 1
}
header() { sed -n "s/^# $1: *//p" "$2" | head -1; }

list_recipes() {
  local d f name seen=" "
  for d in "$OVERLAY/queries" "$SKILL_DIR/queries"; do
    [ -d "$d" ] || continue
    local origin=fleet; [ "$d" = "$OVERLAY/queries" ] && origin=repo
    for f in "$d"/*.logql "$d"/*.promql; do
      [ -f "$f" ] || continue
      name="$(basename "${f%.*}")"
      case "$seen" in *" $name "*) continue ;; esac
      seen="$seen$name "
      printf '  %-22s %-5s %-6s %s\n' "$name" "$origin" "$(header kind "$f")" "$(header desc "$f")"
    done
  done
  if [ -d "$OVERLAY/commands" ]; then
    for f in "$OVERLAY/commands"/*; do
      if [ -f "$f" ] && [ -x "$f" ]; then
        printf '  %-22s %-5s %-6s %s\n' "$(basename "$f")" repo cmd "$(sed -n 's/^# desc: *//p' "$f" | head -1)"
      fi
    done
  fi
}

# --- subcommands --------------------------------------------------------------------
cmd_logs() {
  local q start end; q="$(expand "$1")"
  start="$(secs "${2:-1 hour ago}")"; end="$(secs "${3:-now}")"
  echo "# $q  [$(hms "$start") -> $(hms "$end")Z]" >&2
  loki_range "$q" "${start}000000000" "${end}000000000" "${4:-100}" | jq -r '
    [ .data.result[] as $s | $s.values[]
      | { ns:(.[0]|tonumber), lvl:($s.stream.severity_text // $s.stream.detected_level // "?"),
          sc:($s.stream.scope_name // "-"), body:.[1] } ]
    | sort_by(.ns)[]
    | "  \((.ns/1000000000)|floor|gmtime|strftime("%m-%dT%H:%M:%S"))Z [\(.lvl)] [\(.sc)] \(.body|gsub("\n";" ")|.[0:240])"'
}

cmd_meta() {
  local q start end; q="$(expand "$1")"
  start="$(secs "${2:-1 hour ago}")"; end="$(secs "${3:-now}")"
  loki_range "$q" "${start}000000000" "${end}000000000" "${4:-3}" \
    | jq '.data.result[] | {stream, sample: (.values[0][1])}'
}

# Group matching lines by level + scope + body, with ids and numbers folded so that
# "job ABC123 failed" and "job XYZ789 failed" count as one, plus one sample `error`.
cmd_top() {
  local q start end; q="$(expand "$1")"
  start="$(secs "${2:-7 days ago}")"; end="$(secs "${3:-now}")"
  echo "# $q  [$(hms "$start") -> $(hms "$end")Z]" >&2
  loki_range "$q" "${start}000000000" "${end}000000000" "${4:-5000}" | jq -r '
    def fold: gsub("[0-9a-f]{8}-[0-9a-f-]{27}"; "<uuid>") | gsub("\\b[0-9A-Z]{11}\\b"; "<id>")
              | gsub("\\b[A-Z]+-[0-9]+\\b"; "<REF>") | gsub("[0-9]+"; "N");
    [ .data.result[] as $s | $s.values[]
      | { key: "[\($s.stream.severity_text // "?")] [\($s.stream.scope_name // "-")] \(.[1]|gsub("\n";" ")|fold|.[0:160])",
          err: ($s.stream.error // $s.stream["error.type"] // $s.stream.error_message // ""),
          ns: (.[0]|tonumber) } ]
    | (length) as $n
    | group_by(.key) | map({n: length, key: .[0].key, err: (map(.err) | map(select(. != "")) | .[0] // ""),
                             last: (map(.ns) | max)})
    | sort_by(-.n)[]
    | "\(.n|tostring|(" " * (6 - length)) + .)  \(.key)  (last \((.last/1000000000)|floor|gmtime|strftime("%m-%dT%H:%MZ")))"
      + (if .err != "" then "\n        error: \(.err|gsub("\n";" ")|.[0:200])" else "" end)'
}

cmd_series() {
  local q; q="$(expand "$1")"
  instant "$LOKI" "$q" "$(secs "${2:-now}")000000000" | jq -r '.data.result[] | "  \(.metric) = \(.value[1])"'
}

cmd_prom() {
  local q; q="$(expand "$1")"
  instant "$PROM" "$q" "$(secs "${2:-now}")" | jq -r '.data.result[] | "  \(.metric|del(.__name__)) = \(.value[1])"'
}

cmd_trace() {
  need_token
  # Not found = not stored: sampled away or past retention. Its log lines still carry
  # the id: obsq.sh logs '{{sel}} | trace_id=`<id>`'
  curl -s -f -H "Authorization: Bearer $TOKEN" "$TEMPO/traces/$1" | jq -r '
    [ (.batches // .resourceSpans)[] as $b
      | ([$b.resource.attributes[]? | select(.key=="service.name") | .value.stringValue][0] // "?") as $svc
      | ($b.scopeSpans // $b.instrumentationLibrarySpans)[].spans[]
      | { start:(.startTimeUnixNano|tonumber), name, svc:$svc,
          ms:((((.endTimeUnixNano|tonumber)-(.startTimeUnixNano|tonumber))/1000000)|floor),
          err:(if (.status.code // "") == "STATUS_CODE_ERROR" or (.status.code // 0) == 2 then " ERROR" else "" end) } ]
    | sort_by(.start)[]
    | "  \((.start/1000000000)|floor|gmtime|strftime("%H:%M:%S"))Z \(.ms)ms\(.err) [\(.svc)] \(.name)"' \
    || echo "obsq: trace $1 not in Tempo (sampled out or expired); its log lines: obsq.sh logs '{{sel}} | trace_id=\`$1\`'" >&2
}

cmd_ref() {
  need_selector
  local start end
  start="$(secs "${2:-24 hours ago}")"; end="$(secs "${3:-now}")"
  # `^<prefix>.*`, both parts: `=~` is anchored (a bare prefix matches nothing), and
  # without the `^` Loki's literal optimisation turns `abc.*` into "contains".
  loki_range "$SELECTOR | trace_id=~\`^$1.*\`" "${start}000000000" "${end}000000000" 1000 \
    | jq -r '[.data.result[].stream.trace_id] | unique[]'
}

cmd_run() {
  local name="${1:-}"; [ -n "$name" ] || die "run: which recipe? (obsq.sh recipes)"; shift
  local f; f="$(recipe_file "$name")" || die "no recipe '$name' (obsq.sh recipes)"
  RANGE="$(header range "$f")"
  local args=()
  while [ $# -gt 0 ]; do
    case "$1" in --range) RANGE="$2"; shift 2 ;; *) args+=("$1"); shift ;; esac
  done
  local q kind; q="$(grep -v '^#' "$f" | sed '/^[[:space:]]*$/d')"
  kind="$(header kind "$f")"
  [ -n "$kind" ] || case "$f" in *.promql) kind=prom ;; *) kind=logs ;; esac
  q="$(expand "$q" "${args[@]+"${args[@]}"}")"
  local since; since="$(header since "$f")"
  # A recipe that names a range but no since looks back over that range.
  [ -z "$since" ] && [ -n "$RANGE" ] && since="$(range_to_since "$RANGE")"
  case "$kind" in
    logs|meta|top) "cmd_$kind" "$q" "${since:-1 hour ago}" now "$(header limit "$f")" ;;
    series|prom) "cmd_$kind" "$q" ;;
    *) die "recipe $f: unknown kind '$kind'" ;;
  esac
}
range_to_since() { # 7d / 12h / 30m -> '7 days ago' etc.
  local n="${1%[smhdw]}" u="${1: -1}"
  case "$u" in s) echo "$n seconds ago" ;; m) echo "$n minutes ago" ;; h) echo "$n hours ago" ;;
               d) echo "$n days ago" ;; w) echo "$n weeks ago" ;; *) echo "1 hour ago" ;; esac
}

cmd_env() {
  cat <<EOF
repo       $REPO_ROOT
overlay    $( [ -d "$OVERLAY" ] && echo "$OVERLAY" || echo "(none: obsq.sh init)")
selector   ${SELECTOR:-(unset)}
service    ${FIRST_SERVICE:-(unset)}
grafana    $GRAFANA_URL  (token: $( [ -n "$TOKEN" ] && echo "from $TOKEN_FROM" || echo MISSING))
uids       loki=$LOKI_UID prometheus=$PROM_UID tempo=$TEMPO_UID
EOF
}

cmd_init() {
  [ -e "$OVERLAY" ] && die "$OVERLAY already exists"
  mkdir -p "$OVERLAY/queries" "$OVERLAY/commands"
  cp "$SKILL_DIR/overlay-template/config" "$SKILL_DIR/overlay-template/NOTES.md" "$OVERLAY/"
  [ -n "$PRODUCT" ] && sed -i.bak "s/<product>/$PRODUCT/g" "$OVERLAY/config" "$OVERLAY/NOTES.md" && rm -f "$OVERLAY"/*.bak
  touch "$OVERLAY/queries/.keep" "$OVERLAY/commands/.keep"
  echo "obsq: scaffolded $OVERLAY; set SERVICES in config, then: obsq.sh env"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  logs|meta|top|series|prom)
    [ -n "${1:-}" ] || die "missing query"; "cmd_$cmd" "$@" ;;
  trace|ref) [ -n "${1:-}" ] || die "missing id"; "cmd_$cmd" "$@" ;;
  run) cmd_run "$@" ;;
  recipes) list_recipes ;;
  env) cmd_env ;;
  init) cmd_init ;;
  ''|-h|--help|help) sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//' ;;
  *)
    if [ -x "$OVERLAY/commands/$cmd" ]; then
      need_token; export OBSQ_TOKEN="$TOKEN"; exec "$OVERLAY/commands/$cmd" "$@"
    fi
    die "unknown subcommand '$cmd' (obsq.sh help)" ;;
esac
