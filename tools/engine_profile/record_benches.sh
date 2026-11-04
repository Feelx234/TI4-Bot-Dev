#!/usr/bin/env bash
# Re-records the six bench baselines (tools/rulebot/baseline*.json) at a rules version with a given selfplay binary.
#
#   tools/engine_profile/record_benches.sh LEGACY_SELFPLAY [WORKERS] [RULES_VERSION=35]
#
# Why: the stored baselines were played at rules version 23 (bench), 18 (bench-v5), 11 (v1..v4) and the native engine plays only
# the newest version, so `verify_benches.sh` cannot compare against them any more. The recorder must be the FROZEN LEGACY engine: a
# selfplay built at core-integration-20 (the last line before the atomic swap; ti4-engine there is the string engine), never the
# binary under test. The new baselines are then verified by the native binary (verify_benches.sh, 200/200 identical x6).
# What it does: sets "rules_version" in golden.json / golden-v2.json / golden-v4.json / golden-v5.json, runs the six benches with the
# same arguments as verify_benches.sh but `--out tools/rulebot/baseline*.json` instead of `--verify`, JOBS (default 3) at a time.
# Output files are written to a temp name and moved into place only when the run exits 0. Nothing is committed.
# Env: FINAL_RV (set the four configs to this rules version after the run), JOBS, BENCH_WORKERS, RB_DIR (a copy of tools/rulebot, tests), RECORD_OUT (logs).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rb="${RB_DIR:-$here/../rulebot}"
bin="${1:?usage: record_benches.sh LEGACY_SELFPLAY [WORKERS] [RULES_VERSION]}"; workers="${2:-6}"; rv="${3:-35}"
[[ -x "$bin" ]] || { echo "not executable: $bin" >&2; exit 2; }
jobs="${JOBS:-3}"; per="${BENCH_WORKERS:-$(( (workers + jobs - 1) / jobs ))}"
out="${RECORD_OUT:-$(mktemp -d)}"; mkdir -p "$out"
export SELFPLAY_RESULTS=off
for c in golden golden-v2 golden-v4 golden-v5; do sed -i -E "s/\"rules_version\": [0-9]+/\"rules_version\": $rv/" "$rb/$c.json"; done
run() {
  local name="$1" file="$2"; shift 2
  "$bin" run "$@" --no-cache --workers "$per" --out "$out/$file.tmp" --quiet --no-table > "$out/$name.log" 2>&1
  local rc=$?
  if [[ $rc == 0 && -s "$out/$file.tmp" ]]; then mv "$out/$file.tmp" "$rb/$file"; echo "$name: recorded $file at rules version $rv"
  else echo "$name: FAILED rc $rc, see $out/$name.log"; fi > "$out/$name.line"
}
start() {
  case "$1" in
    bench)    run bench    baseline.json    --config "$rb/golden.json" ;;
    bench-v5) run bench-v5 baseline-v5.json --config "$rb/golden-v5.json" --bot bot_version=5 ;;
    bench-v4) run bench-v4 baseline-v4.json --config "$rb/golden-v4.json" --bot bot_version=4 ;;
    bench-v1) run bench-v1 baseline-v1.json --config "$rb/golden-v2.json" --bot bot_version=1 ;;
    bench-v2) run bench-v2 baseline-v2.json --config "$rb/golden-v2.json" --bot bot_version=2 ;;
    bench-v3) run bench-v3 baseline-v3.json --config "$rb/golden-v2.json" --bot bot_version=3 ;;
  esac
}
names=(bench bench-v5 bench-v4 bench-v1 bench-v2 bench-v3)
[[ -n "${ONLY:-}" ]] && read -ra names <<< "$ONLY"
rm -f "$out"/*.line; running=0
for n in "${names[@]}"; do
  if (( running >= jobs )); then wait -n; running=$((running - 1)); fi
  start "$n" &
  running=$((running + 1))
done
wait
# FINAL_RV: the rules version the committed configs must carry afterwards. The recorder is the frozen string engine (rules version 35),
# the engine under test plays only the newest one (36): the configs are set back to FINAL_RV so that verify_benches.sh can run them.
[[ -n "${FINAL_RV:-}" ]] && for c in golden golden-v2 golden-v4 golden-v5; do sed -i -E "s/\"rules_version\": [0-9]+/\"rules_version\": $FINAL_RV/" "$rb/$c.json"; done
bad=0
for n in "${names[@]}"; do cat "$out/$n.line" 2>/dev/null || { echo "$n: no result"; bad=1; }; done
grep -q FAILED "$out"/*.line 2>/dev/null && bad=1
exit $bad
