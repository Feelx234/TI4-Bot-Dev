#!/usr/bin/env bash
# Print recent playtest feedback reports in the terminal, newest first.
#
#   tools/public/list_feedback.sh [-n 20] [-v] [DATA_DIR]
#
# -n N  how many reports (default 20)      -v  also print the full context JSON
# DATA_DIR defaults to $TI4_DATA_DIR or /var/lib/ti4-public.
# Uses the server binary when present, otherwise falls back to jq over the JSON files.
set -euo pipefail

limit=20
verbose=""
while getopts "n:v" opt; do
  case "$opt" in
    n) limit="$OPTARG" ;;
    v) verbose="--verbose" ;;
    *) echo "usage: $0 [-n N] [-v] [DATA_DIR]" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
data_dir="${1:-${TI4_DATA_DIR:-/var/lib/ti4-public}}"

for bin in /opt/ti4-public/current/server "${TI4_SERVER_BIN:-}"; do
  if [[ -n "$bin" && -x "$bin" ]]; then
    exec "$bin" feedback-list --data-dir "$data_dir" --limit "$limit" $verbose
  fi
done

command -v jq >/dev/null || { echo "need the server binary or jq" >&2; exit 1; }
# shellcheck disable=SC2012
ls -1t "$data_dir"/feedback/*/*.json 2>/dev/null | head -n "$limit" | while read -r f; do
  jq -r '"=== \(.id) | \(.received_at) | \(.category | ascii_upcase) | client \(.context.build_commit // "-") | game \(.context.game_id // "-") | seat \(.context.viewer_seat // "-") | round \(.context.round // "-") \(.context.phase // "")\n\(if .context.sender or .context.players then "    from \(.context.sender.nickname // .context.sender.typed_name // (if .context.sender.lobby_nickname then "lobby name " + .context.sender.lobby_nickname else "?" end))" + (if .context.players then " | table: " + ([.context.players[] | "\(.nickname // "?")\(if .is_sender then "*" else "" end) (\(.faction // "?"))"] | join(", ")) else "" end) + "\n" else "" end)\(.text)\n    file: '"$f"'\n"' "$f"
  if [[ -n "$verbose" ]]; then jq '.context' "$f"; fi
done
