#!/usr/bin/env bash
# The native playthrough (lane M7, docs/M7_RESUME.md): whole games played by the CORE ALONE, every gap counted.
#
#   CARGO_TARGET_DIR=<private dir> tools/engine_core/native_play.sh [NATIVE_PLAY_ARGS...]
#
# Builds the `native_play` bin with the `gate` profile (optimised, no LTO) and runs it; with no argument it is the quick check:
# faction sol, 4 players, seed 1. Arguments are passed on:
#
#   ... native_play.sh --factions all --players 3,4,5,6 --seeds 1     every lobby faction at every table size (the Z-gate's entry table)
#   ... native_play.sh --quick --shadow                               run the legacy game in parallel and compare every step
#   ... native_play.sh --quick --survey                               the legacy game leads a whole game; every step is classified by the arm
#                                                                     and question source the native loop would need (volumes of the gaps)
#   ... native_play.sh --quick --twice                                determinism: the second run must give the same fingerprint
#
# Exit 0 only when every game finished natively with zero gaps (the Z-gate waits for 8 consecutive green seeds over all lobby factions).
# Not a release build (the user rule: no release builds); `gate` is the profile of docs/ENGINE_BUILD_SPEED.md.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  echo "CARGO_TARGET_DIR is required (a private directory, e.g. /root/deploytmp/<lane>)" >&2
  exit 2
fi
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" CARGO_INCREMENTAL=0
cd "$repo"
cargo build --profile gate -p ti4-engine-core-bridge --bin native_play
if [[ $# -eq 0 ]]; then set -- --quick; fi
exec "$CARGO_TARGET_DIR/gate/native_play" "$@"
