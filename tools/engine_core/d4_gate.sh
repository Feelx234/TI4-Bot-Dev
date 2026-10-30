#!/usr/bin/env bash
# The D4 gate: the typed core verified WITHOUT the string engine (docs/todo/D4-core-tests.md 1, 6.3).
#
#   tools/engine_core/d4_gate.sh [STEP...]        default: every step; STEP is one of 1..6
#
# Steps (a step that is not built yet prints SKIP and does not fail):
#   1  cargo test of the core, the factions crate and the D4 test crate (dev-like profile: debug assertions on)
#   2  layers and hygiene of the core crates (check_layers.py core, lint_core.sh)
#   3  the default build of the D4 test crate holds no engine at all (cargo tree)
#   4  coverage report and the covers-gap check (tools/engine_core/d4/coverage.py --check)       SKIP until D4-5
#   5  frozen golden replays (golden check)                                                        SKIP until D4-4
#   6  mutation floors (tools/engine_core/mutation_ratchet.sh)                                     SKIP until D4-6
# Heavy: take /root/.ti4-heavy.lock for the whole run (`flock /root/.ti4-heavy.lock tools/engine_core/d4_gate.sh`).
set -uo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo" || exit 2
steps=("$@"); [[ ${#steps[@]} == 0 ]] && steps=(1 2 3 4 5 6)
fail=0
cfg=()
for p in ti4-engine-core ti4-engine-core-factions ti4-engine-core-tests; do
  cfg+=(--config "profile.lockstep.package.$p.debug-assertions=true" --config "profile.lockstep.package.$p.overflow-checks=true")
done
for s in "${steps[@]}"; do
  case "$s" in
    1)
      echo "== 1 cargo test (core, factions, d4 tests)"
      if cargo test --offline --profile lockstep "${cfg[@]}" -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-tests; then echo "step 1: ok"; else echo "step 1: FAILED"; fail=1; fi ;;
    2)
      echo "== 2 layers and hygiene"
      if python3 -I tools/engine_core/check_layers.py core && tools/engine_core/lint_core.sh; then echo "step 2: ok"; else echo "step 2: FAILED"; fail=1; fi ;;
    3)
      echo "== 3 no engine in the default build of the d4 tests"
      n="$(cargo tree --offline -p ti4-engine-core-tests -e normal,dev --prefix none 2>/dev/null | grep -cE '^ti4-engine(-legacy)? v')"
      if [[ "$n" == "0" ]]; then echo "step 3: ok (0 engine crates)"; else echo "step 3: FAILED ($n engine crates in the default build)"; fail=1; fi ;;
    4)
      if [[ -f tools/engine_core/d4/coverage.py ]]; then
        echo "== 4 coverage"; if python3 -I tools/engine_core/d4/coverage.py --check; then echo "step 4: ok"; else echo "step 4: FAILED"; fail=1; fi
      else echo "== 4 coverage: SKIP (not built yet)"; fi ;;
    5)
      if [[ -x tools/engine_core/golden_check.sh ]]; then
        echo "== 5 golden"; if tools/engine_core/golden_check.sh; then echo "step 5: ok"; else echo "step 5: FAILED"; fail=1; fi
      else echo "== 5 golden: SKIP (not built yet)"; fi ;;
    6)
      if [[ -x tools/engine_core/mutation_ratchet.sh ]]; then
        echo "== 6 mutation floors"; if tools/engine_core/mutation_ratchet.sh; then echo "step 6: ok"; else echo "step 6: FAILED"; fail=1; fi
      else echo "== 6 mutation floors: SKIP (not built yet)"; fi ;;
    *) echo "unknown step $s"; fail=1 ;;
  esac
done
exit $fail
