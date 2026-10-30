#!/usr/bin/env bash
# Run the D4 core tests of one commit in a persistent scratch worktree with one private target dir, print a digest.
#
#   tools/engine_core/d4/run_core_tests.sh <commit> [BLESS=<text>|BLESS=1] [-- extra cargo test args / filters]
#
# * Profile: `lockstep` with debug assertions and overflow checks ON for the three core crates (the unloaded-slice guard of
#   `CoreState` is a `debug_assert!`: trap T9 of docs/todo/D4-core-tests.md), so the D4 tests see what the dev profile sees.
# * Never builds while another agent holds /root/.ti4-heavy.lock: queue with `HAVE_LOCK=1 flock /root/.ti4-heavy.lock <this script> <ref>`.
# * BLESS=<text> blesses the scenario snapshots whose name contains <text> and copies tests/snapshots back into $LIVE.
set -u
sha="$1"; shift
blessval=""
if [[ "${1:-}" == BLESS=* ]]; then blessval="${1#BLESS=}"; shift; fi
[[ "${1:-}" == "--" ]] && shift
REPO="${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
LIVE="${LIVE:-$REPO}"
WT="${WT:-/root/deploytmp/core-d4a-wt}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/root/deploytmp/core-d4a}" CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export D4_COVERS_FILE="${D4_COVERS_FILE:-/root/deploytmp/core-d4a-covers.txt}"
LOGS=/root/deploytmp/core-d4a-logs
mkdir -p "$LOGS"
# Queue for the heavy lock like every other job: `HAVE_LOCK=1 flock /root/.ti4-heavy.lock tools/engine_core/d4/run_core_tests.sh <ref> ...`
# (a ref such as `core-d4a` is resolved after the lock is taken, so the newest commit is tested). Without HAVE_LOCK: refuse while it is held.
if [ -z "${HAVE_LOCK:-}" ]; then if ! flock -n /root/.ti4-heavy.lock true; then echo "HEAVY LOCK HELD: wait"; exit 3; fi; fi
sha="$(git -C "$REPO" rev-parse "$sha")"
if [ ! -d "$WT" ]; then git -C "$REPO" worktree add --detach "$WT" "$sha" >/dev/null 2>&1; else git -C "$WT" checkout -q --detach "$sha" || exit 4; fi
cd "$WT" || exit 4
log="$LOGS/$sha.log"
cfg=()
for p in ti4-engine-core ti4-engine-core-factions ti4-engine-core-tests; do
  cfg+=(--config "profile.lockstep.package.$p.debug-assertions=true" --config "profile.lockstep.package.$p.overflow-checks=true")
done
[[ -n "$blessval" ]] && export BLESS="$blessval"
# One pass that reports every compile error of every test target, then the tests.
nice -n 10 cargo build --offline --tests --keep-going --profile lockstep "${cfg[@]}" -p ti4-engine-core-tests >"$log" 2>&1
rc=$?
if [ $rc -eq 0 ]; then
  nice -n 10 cargo test --offline --profile lockstep "${cfg[@]}" -p ti4-engine-core-tests --no-fail-fast "$@" >>"$log" 2>&1
  rc=$?
fi
echo "exit: $rc (full log: $log)"
grep -E "^test result|^test .* FAILED|^failures:|Running |error(\[|:)|panicked at|could not compile" "$log" | head -80
echo "--- first errors ---"
grep -E -A12 "^error" "$log" | head -150
echo "--- failure details ---"
grep -E -A14 "^---- .* stdout ----" "$log" | head -200
if [[ -n "$blessval" && -d "$WT/crates/ti4-engine-core-tests/tests/snapshots" ]]; then
  rsync -a "$WT/crates/ti4-engine-core-tests/tests/snapshots/" "$LIVE/crates/ti4-engine-core-tests/tests/snapshots/"
fi
cp "$WT/Cargo.lock" "$LIVE/Cargo.lock"
