#!/usr/bin/env bash
# D3-0 item 4: the single baseline run of the string bot at the base commit, under the heavy lock, then base.json.
#
#   tools/rulebot/d3/baseline.sh BASE_REV SCRATCH_DIR   e.g. 5557c60a /root/deploytmp/core-d3
#
# Builds BASE_REV in a scratch worktree (profile gate), saves SCRATCH_DIR/base-selfplay (the base binary of the D3-10 CPU
# A/B), plays the golden set with --verify baseline.json (must say 200 identical) and writes tools/rulebot/d3/base.json
# from the run and view_calls.json. Blocks on /root/.ti4-heavy.lock; deletes the scratch build when done.
set -euo pipefail
rev="$1"; scratch="$2"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
wt="$scratch/base-wt"; mkdir -p "$scratch"
exec 9>/root/.ti4-heavy.lock; flock 9
git -C "$repo" worktree add --detach "$wt" "$rev"
export CARGO_TARGET_DIR="$scratch/target" CARGO_BUILD_JOBS=2 SELFPLAY_RESULTS=off
(cd "$wt" && nice -n 10 cargo build --profile gate -p ti4-selfplay)
cp "$CARGO_TARGET_DIR/gate/selfplay" "$scratch/base-selfplay"
(cd "$wt" && /usr/bin/time -f "%U %S %e" -o "$scratch/time" nice -n 5 "$scratch/base-selfplay" run --config tools/rulebot/golden.json \
  --no-cache --workers 4 --out "$scratch/bench.json" --verify tools/rulebot/baseline.json)
python3 -I "$here/make_base.py" "$scratch/bench.json" "$scratch/time" "$here/view_calls.json" "$rev" > "$here/base.json"
git -C "$repo" worktree remove --force "$wt"; rm -rf "$CARGO_TARGET_DIR"
