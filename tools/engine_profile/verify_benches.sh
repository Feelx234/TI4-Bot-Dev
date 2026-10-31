#!/usr/bin/env bash
# Every bench of tools/rulebot/selfplay.sh (bench, bench-v1..v5) with a given selfplay binary,
# with CPU time per bench: each must print "200 identical, 0 different".
#
#   tools/engine_profile/verify_benches.sh BIN [WORKERS]
#
# The six benches run CONCURRENTLY, JOBS at a time (env JOBS, default 3; JOBS=1 is the old sequential
# script). WORKERS (default 6) is the TOTAL worker budget: each bench gets ceil(WORKERS / JOBS) workers
# (env BENCH_WORKERS overrides that), so the CPU use stays what it was and the wall time loses the tail of
# the slowest bench and the serial start-up of each. The results do not depend on the worker count or on
# the job count (the decision hashes are per game). The output is the same six lines as always, printed in
# the fixed order bench, bench-v5, bench-v4, bench-v1, bench-v2, bench-v3 when all have finished (the
# callers, tools/engine_core/verify_package.sh and verify_stage.sh, count the "200 identical, 0 different").
# `cpu` is the user+sys time of that bench's own process; `wall` is its own elapsed time (longer than
# alone when the benches share the cores).
# Env: JOBS, BENCH_WORKERS, VERIFY_OUT (logs; default a temp dir), HEAVY_LOCK=1 (take /root/.ti4-heavy.lock
# for the whole run; the default is not to, because the gates that call this script already hold it and a
# second lock in a child would wait for its own parent).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${HEAVY_LOCK:-0}" == 1 && -z "${TI4_HEAVY_LOCK_TAKEN:-}" ]]; then
  export TI4_HEAVY_LOCK_TAKEN=1
  exec flock /root/.ti4-heavy.lock "${BASH_SOURCE[0]}" "$@"
fi
rb="${RB_DIR:-$here/../rulebot}"   # RB_DIR: a directory with smaller copies of the configs (tests)
bin="$1"; workers="${2:-6}"
jobs="${JOBS:-3}"
if ! [[ "$jobs" =~ ^[0-9]+$ ]] || (( jobs < 1 )); then echo "JOBS must be a positive integer (got '$jobs')" >&2; exit 2; fi
per="${BENCH_WORKERS:-$(( (workers + jobs - 1) / jobs ))}"
out="${VERIFY_OUT:-$(mktemp -d)}"; mkdir -p "$out"
export SELFPLAY_RESULTS=off
names=(bench bench-v5 bench-v4 bench-v1 bench-v2 bench-v3)
run() {
  local name="$1"; shift
  /usr/bin/time -f "%U %S %e" -o "$out/$name.time" "$bin" run "$@" --no-cache --workers "$per" \
    --out "$out/$name.json" --quiet --no-table > "$out/$name.log" 2>&1
  local rc=$?
  # `time -o` puts "Command exited with non-zero status N" in front of the numbers when the bench exits non-zero (selfplay exits 2
  # on a difference): read the last line, else the "different" line came out as "cpu  s wall with non-zero status 2 s" and bc failed.
  read -r u s e < <(tail -n 1 "$out/$name.time")
  echo "$name: $(grep -o '[0-9]* identical, [0-9]* different, [0-9]* not in the baseline' "$out/$name.log" || echo "rc $rc, see $out/$name.log") | cpu $(echo "$u + $s" | bc) s wall $e s" > "$out/$name.line"
}
start() {
  case "$1" in
    bench)    run bench    --config "$rb/golden.json"                         --verify "$rb/baseline.json" ;;
    bench-v5) run bench-v5 --config "$rb/golden-v5.json" --bot bot_version=5  --verify "$rb/baseline-v5.json" ;;
    bench-v4) run bench-v4 --config "$rb/golden-v4.json" --bot bot_version=4  --verify "$rb/baseline-v4.json" ;;
    bench-v1) run bench-v1 --config "$rb/golden-v2.json" --bot bot_version=1  --verify "$rb/baseline-v1.json" ;;
    bench-v2) run bench-v2 --config "$rb/golden-v2.json" --bot bot_version=2  --verify "$rb/baseline-v2.json" ;;
    bench-v3) run bench-v3 --config "$rb/golden-v2.json" --bot bot_version=3  --verify "$rb/baseline-v3.json" ;;
  esac
}
# A subset for a quick check of the script itself: ONLY="bench bench-v5".
[[ -n "${ONLY:-}" ]] && read -ra names <<< "$ONLY"
rm -f "$out"/*.line
running=0
for n in "${names[@]}"; do
  if (( running >= jobs )); then wait -n; running=$((running - 1)); fi
  start "$n" &
  running=$((running + 1))
done
wait
for n in "${names[@]}"; do cat "$out/$n.line" 2>/dev/null || echo "$n: no result (see $out/$n.log)"; done
