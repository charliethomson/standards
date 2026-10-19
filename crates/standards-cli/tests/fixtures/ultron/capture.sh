#!/bin/bash
# Recapture the goldens: bin/ultron's output over a scratch copy of these fixtures, with
# `date +%s` pinned to ./now and each file's mtime set from ./mtimes.tsv. Paths print as
# @STATE@ (the tests substitute their own scratch dir). Never touches ~/.local/state.
set -euo pipefail
FIX="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$FIX/../../../../.." && pwd)"
SCR="$(mktemp -d)"
trap 'rm -rf "$SCR"' EXIT
mkdir -p "$SCR/state/thmsn/ultron" "$SCR/shim"
NOW=$(cat "$FIX/now")
PROGRAMS=(standards-hosted fleet-backups stash tav-backlog-audit)

cat > "$SCR/shim/date" <<SH
#!/bin/bash
if [ "\${1:-}" = "+%s" ]; then echo $NOW; else exec /bin/date "\$@"; fi
SH
chmod +x "$SCR/shim/date"

for p in "${PROGRAMS[@]}"; do cp -R "$FIX/$p" "$SCR/state/thmsn/ultron/$p"; done
while IFS=$'\t' read -r f m; do
  perl -e 'utime $ARGV[0], $ARGV[0], $ARGV[1] or die "$ARGV[1]: $!"' "$m" "$SCR/state/thmsn/ultron/$f"
done < "$FIX/mtimes.tsv"

mkdir -p "$FIX/golden"
run() { PATH="$SCR/shim:$PATH" XDG_STATE_HOME="$SCR/state" bash "$ROOT/bin/ultron" "$@"; }
run ls > "$FIX/golden/ls.txt"
run status > "$FIX/golden/status.txt"
run --help > "$FIX/golden/help.txt"
run log STASH-9 > "$FIX/golden/log-STASH-9.txt"
for p in "${PROGRAMS[@]}"; do run status "$p" > "$FIX/golden/status-$p.txt"; done
perl -pi -e "s|\Q$SCR/state\E|\@STATE\@|g" "$FIX"/golden/*.txt
