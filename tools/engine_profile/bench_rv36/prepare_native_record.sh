#!/usr/bin/env bash
# Bench stops lane (docs/BENCH_STOPS_RESUME.md): the native re-recording of the six bench baselines for rules version 36, PREPARED in a copy.
#
#   tools/engine_profile/bench_rv36/prepare_native_record.sh NATIVE_SELFPLAY OUTDIR [WORKERS]
#
# Copies tools/rulebot to OUTDIR, records the six baselines there with the NATIVE binary at rules version 36 (record_benches.sh, configs set to 36),
# then verifies them with the same binary (verify_benches.sh: must print "200 identical, 0 different" six times, which also proves the native engine is
# deterministic). tools/rulebot itself is NOT touched. To adopt the result, in ONE commit:
#   cp OUTDIR/baseline*.json OUTDIR/golden*.json tools/rulebot/ && git add tools/rulebot && git commit
# (the configs carry rules_version 36 in the copy). Needs a heavy slot: run it as `/root/heavy2.sh <this script> ...`.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="${1:?usage: prepare_native_record.sh NATIVE_SELFPLAY OUTDIR [WORKERS]}"; out="${2:?}"; workers="${3:-6}"
mkdir -p "$out"
cp "$here/../../rulebot/"*.json "$out/"
RB_DIR="$out" FINAL_RV=36 RECORD_OUT="$out/record-logs" "$here/../record_benches.sh" "$bin" "$workers" 36 || { echo "recording failed"; exit 1; }
RB_DIR="$out" VERIFY_OUT="$out/verify-logs" "$here/../verify_benches.sh" "$bin" "$workers"
