#!/usr/bin/env bash
# Checks that a server binary can load every saved game, without touching the live data.
#
#   recovery-check.sh preflight --bin SERVER --web-dir DIST --data-dir LIVE_DIR [--allow-failed ID]...
#       Copies LIVE_DIR (without feedback/) to a temp dir, starts SERVER on a free loopback port
#       against the copy, waits for the "Durable storage: recovered N game(s); F failed; R refused"
#       line, stops it and compares with the number of game_* directories in LIVE_DIR. Exit 0 only
#       when every game recovered or was REFUSED CLEANLY, except the ids named with --allow-failed.
#       A refusal is a save the product does not play on purpose (a rules version below the newest,
#       Thunder's Edge content, a dropped faction or feature: the server logs "refused 'ID'" with
#       the reason and keeps the files); the check requires that every refusal names its reason,
#       and counts it apart from a failure. Prints the failed ids, one per line, as
#       "FAILED_IDS <ids>" and the refused ones as "REFUSED_IDS <ids>" for the caller.
#   recovery-check.sh parse [FILE]
#       Reads a server log (FILE or stdin), prints
#       "recovered=N failed=F refused=R refused_ids=<id,id> ids=<id,id>" for the LAST recovery
#       summary in it (`ids` = the failed ones, last). A summary of an older server has no refused
#       count: refused=0. Exit 1 when there is none.
#   recovery-check.sh compare "<ids of the expected failures>" "<ids that failed>"
#       Exit 1 and print the unexpected ones when the second list is not a subset of the first.
#
# Why: a checksum change once made all 22 saved games unrecoverable; it was noticed only after the
# restart. The server keeps the files of games it cannot load, so the check is safe either way.
set -euo pipefail

parse_log() { # stdin: log; prints "recovered=N failed=F refused=R refused_ids=a,b ids=c,d"
  awk '
    /Durable storage: could not recover/ {
      if (match($0, /could not recover \047[^\047]+\047/)) {
        id = substr($0, RSTART + 19, RLENGTH - 20); cur = cur (cur == "" ? "" : ",") id
      }
    }
    /Durable storage: refused/ {
      if (match($0, /refused \047[^\047]+\047/)) {
        id = substr($0, RSTART + 9, RLENGTH - 10); ref = ref (ref == "" ? "" : ",") id
      }
    }
    /Durable storage: recovered [0-9]+ game\(s\); [0-9]+ failed/ {
      n = 0
      for (i = 1; i <= NF; i++) if ($i == "recovered") r = $(i + 1)
      for (i = 1; i <= NF; i++) if ($i ~ /^failed;?$/ && i > 1 && $(i - 1) ~ /^[0-9]+$/) f = $(i - 1)
      for (i = 2; i <= NF; i++) if ($i == "refused" && $(i - 1) ~ /^[0-9]+$/) n = $(i - 1)
      out = "recovered=" r " failed=" f " refused=" n " refused_ids=" ref " ids=" cur; found = 1; cur = ""; ref = ""
    }
    END { if (found) print out; else exit 1 }
  '
}

free_port() {
  python3 - <<'P'
import socket
s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])
P
}

subset_check() { # $1 = allowed (comma list), $2 = actual (comma list)
  local bad="" id
  IFS=',' read -ra actual <<<"$2"
  for id in "${actual[@]}"; do
    [[ -z "$id" ]] && continue
    case ",$1," in *",$id,"*) ;; *) bad="$bad $id" ;; esac
  done
  if [[ -n "$bad" ]]; then echo "unexpected failures:$bad"; return 1; fi
}

cmd="${1:-}"; shift || true
case "$cmd" in
  parse) parse_log <"${1:-/dev/stdin}" ;;
  compare) subset_check "${1:-}" "${2:-}" ;;
  preflight)
    bin="" web="" live="" allow=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --bin) bin="$2"; shift 2 ;;
        --web-dir) web="$2"; shift 2 ;;
        --data-dir) live="$2"; shift 2 ;;
        --allow-failed) allow="${allow:+$allow,}$2"; shift 2 ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
      esac
    done
    [[ -x "$bin" && -d "$web" && -d "$live" ]] || { echo "need --bin (executable), --web-dir, --data-dir" >&2; exit 2; }
    work="$(mktemp -d "${TMPDIR:-/tmp}/ti4-preflight.XXXXXX")"
    pid=""
    cleanup() { [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$work"; }
    trap cleanup EXIT
    mkdir "$work/data"
    # Copy as the current user; sudo only when the live dir is not readable.
    if [[ -r "$live" ]] && ! find "$live" -maxdepth 2 ! -readable 2>/dev/null | grep -q .; then
      (cd "$live" && find . -mindepth 1 -maxdepth 1 ! -name feedback -exec cp -a {} "$work/data/" \;)
    else
      (cd "$live" && sudo find . -mindepth 1 -maxdepth 1 ! -name feedback -exec cp -a {} "$work/data/" \;)
      sudo chown -R "$(id -u):$(id -g)" "$work/data"
    fi
    expected="$(find "$work/data" -mindepth 1 -maxdepth 1 -type d -name 'game_*' | wc -l)"
    port="$(free_port)"
    echo "preflight: $expected saved game(s), copy in $work/data, port $port"
    # A throwaway public-mode config: nothing here is the live secret.
    env -i PATH="$PATH" HOME="$work" HOST=127.0.0.1 PORT="$port" TI4_PUBLIC=1 \
      TI4_ACCESS_PASSWORD=preflight TI4_COOKIE_SECRET=preflight-not-a-secret \
      TI4_DATA_DIR="$work/data" TI4_WEB_DIR="$web" TI4_RATE_LIMITS=off \
      "$bin" >"$work/server.log" 2>&1 &
    pid=$!
    summary=""
    for _ in $(seq 1 180); do
      if summary="$(parse_log <"$work/server.log" 2>/dev/null)"; then break; fi
      kill -0 "$pid" 2>/dev/null || { echo "preflight: server exited early:" >&2; tail -20 "$work/server.log" >&2; exit 1; }
      sleep 1
    done
    [[ -n "$summary" ]] || { echo "preflight: no recovery summary after 180 s" >&2; tail -20 "$work/server.log" >&2; exit 1; }
    kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; pid=""
    echo "preflight: $summary"
    rec="${summary#recovered=}"; rec="${rec%% *}"
    rest="${summary#* failed=}"; fail="${rest%% *}"
    rest="${summary#* refused=}"; refused="${rest%% *}"
    rest="${summary#* refused_ids=}"; refused_ids="${rest%% ids=*}"; refused_ids="${refused_ids%% *}"
    ids="${summary##* ids=}"
    echo "FAILED_IDS ${ids//,/ }"
    echo "REFUSED_IDS ${refused_ids//,/ }"
    if (( rec + fail + refused != expected )); then
      echo "preflight FAILED: recovered $rec + failed $fail + refused $refused != $expected game directories" >&2; exit 1
    fi
    # Every refusal must be a clean one: named, with its reason, from the "refused" lines.
    if (( refused > 0 )); then
      unclear="$(grep 'Durable storage: refused' "$work/server.log" | grep -v -E 'older version|switched off' || true)"
      if [[ -n "$unclear" ]]; then
        echo "preflight FAILED: a refusal without a reason:" >&2; echo "$unclear" >&2; exit 1
      fi
      echo "preflight: $refused game(s) refused cleanly (files kept), not counted as failures"
    fi
    if ! subset_check "$allow" "$ids"; then
      echo "preflight FAILED: $fail game(s) cannot be recovered by this build" >&2
      grep "could not recover" "$work/server.log" >&2 || true
      exit 1
    fi
    echo "preflight OK: $rec recovered, $refused refused cleanly, $fail allowed failure(s)"
    ;;
  *) sed -n '2,20p' "$0" >&2; exit 2 ;;
esac
