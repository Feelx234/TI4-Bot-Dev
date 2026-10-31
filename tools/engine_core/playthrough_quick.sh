#!/usr/bin/env bash
# The cheap one-game check of an island lane (docs/D2_LF_RESUME.md, "Running the playthrough").
#
#   CARGO_TARGET_DIR=<private dir> tools/engine_core/playthrough_quick.sh [PLAYTHROUGH_ARGS...]
#
# Builds the `playthrough` bin with the `gate` profile (optimised, no LTO: a few minutes, and a whole game runs about two orders of
# magnitude faster than in a debug build, which needs more than 17 CPU minutes for 2500 steps under all=dual; the decision hashes do
# not depend on the profile) and runs ONE game: `playthrough --quick` = faction sol, 4 players, seed 1, 2500 steps. Arguments are
# passed on, e.g.
#
#   ... playthrough_quick.sh --factions hacan --players 6 --first-seed 3      another faction / table / seed
#   ... playthrough_quick.sh --maxsteps 600 --top 5                           an even cheaper look, more causes per island
#   ... playthrough_quick.sh --areas driver=dual,trade=core                   other areas than all=dual
#
# Read the table "bails by island": your island's row is green (0) or lists its causes, biggest first. A zero count with zero live
# steps of your arm in "coverage" proves nothing: the game must have reached your island. The heavy Z-gate (every lobby faction, 4
# and 6 players, 8 consecutive green seeds) is NOT this script: it waits until the cheap check is green.
#
# Not a release build (the user rule: no release builds); `gate` is the profile of docs/ENGINE_BUILD_SPEED.md. Incremental builds
# are off so the private target dir stays small (it is yours: delete it when done).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  echo "CARGO_TARGET_DIR is required (a private directory, e.g. /root/deploytmp/<lane>)" >&2
  exit 2
fi
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" CARGO_INCREMENTAL=0
cd "$repo"
cargo build --profile gate -p ti4-engine-core-bridge --bin playthrough
exec "$CARGO_TARGET_DIR/gate/playthrough" --quick "$@"
