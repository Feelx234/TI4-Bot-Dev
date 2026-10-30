#!/usr/bin/env bash
# Steps 4-6 of the D4 gate (docs/todo/D4-core-tests.md 1 and 8: coverage --check, golden check, mutation ratchet), for
# tools/engine_core/d4_gate.sh to call. Each step prints `SKIP (...)` when its prerequisite is not there yet.
#
#   tools/engine_core/d4/gate_b.sh [SOAK_CENSUS]      SOAK_CENSUS default tools/engine_core/d4/census/soak-2026-10-29.json
#   env: GOLDEN_BIN (built `golden`, else cargo run), RATCHET=0 skips the mutation step (it takes ~1 h), MASKS ("all=core all=string")
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"; cd "$repo"
census="${1:-$here/census/soak-2026-10-29.json}"
fail=0
echo "== 4 coverage"
python3 -I "$here/coverage.py" selftest || fail=1
python3 -I "$here/coverage.py" --soak "$census" --check --stage-min "${STAGE_MIN:-80}" || fail=1
echo "== 5 golden check"
golden="${GOLDEN_BIN:-}"
if [[ -z "$golden" || ! -x "$golden" ]]; then echo "SKIP (set GOLDEN_BIN to a built golden binary: cargo build --profile lockstep -p ti4-engine-core-bridge --bin golden)"; else
  for m in ${MASKS:-all=core all=string}; do "$golden" check --mask "$m" || fail=1; done
fi
echo "== 6 mutation ratchet"
if [[ "${RATCHET:-1}" == 0 ]]; then echo "SKIP (RATCHET=0)"; else
  [[ -s "$here/../mutation_floor.json" ]] || echo "SKIP (no floors yet)"
  [[ -s "$here/../mutation_floor.json" ]] && { "$here/../mutation_ratchet.sh" || fail=1; }
fi
exit $fail
