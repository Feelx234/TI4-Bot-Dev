#!/usr/bin/env bash
# Every bench of tools/rulebot/selfplay.sh (bench, bench-v1..v5) with a given selfplay binary,
# with CPU time per bench: each must print "200 identical, 0 different".
#
#   tools/engine_profile/verify_benches.sh BIN [WORKERS]
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rb="$here/../rulebot"
bin="$1"; workers="${2:-6}"
out="${VERIFY_OUT:-$(mktemp -d)}"; mkdir -p "$out"
export SELFPLAY_RESULTS=off
run() {
  local name="$1"; shift
  /usr/bin/time -f "%U %S %e" -o "$out/$name.time" "$bin" run "$@" --no-cache --workers "$workers" \
    --out "$out/$name.json" --quiet --no-table > "$out/$name.log" 2>&1
  local rc=$?
  read -r u s e < "$out/$name.time"
  echo "$name: $(grep -o '[0-9]* identical, [0-9]* different, [0-9]* not in the baseline' "$out/$name.log" || echo "rc $rc, see $out/$name.log") | cpu $(echo "$u + $s" | bc) s wall $e s"
}
run bench    --config "$rb/golden.json"                         --verify "$rb/baseline.json"
run bench-v5 --config "$rb/golden-v5.json" --bot bot_version=5  --verify "$rb/baseline-v5.json"
run bench-v4 --config "$rb/golden-v4.json" --bot bot_version=4  --verify "$rb/baseline-v4.json"
run bench-v1 --config "$rb/golden-v2.json" --bot bot_version=1  --verify "$rb/baseline-v1.json"
run bench-v2 --config "$rb/golden-v2.json" --bot bot_version=2  --verify "$rb/baseline-v2.json"
run bench-v3 --config "$rb/golden-v2.json" --bot bot_version=3  --verify "$rb/baseline-v3.json"
