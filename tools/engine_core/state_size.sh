#!/usr/bin/env bash
# Gate 10: size and clone cost of the state kinds, with allocation counts from mimalloc's counters
# (the workspace forbids an allocator hook). Needs $CARGO_TARGET_DIR with `state_size` built
# (cargo build --release -p ti4-engine-core-bridge --bins).
#
#   tools/engine_core/state_size.sh [CONFIG] [GAMES] [STRIDE] [REPS]
set -uo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bin="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/gate/state_size"
config="${1:-$repo/tools/engine_profile/profile-20.json}"; games="${2:-2}"; stride="${3:-50}"; reps="${4:-20}"
"$bin" "$config" "$games" "$stride" "$reps"
# Allocator counters: the difference between a run that clones and one that does not.
stat() { MIMALLOC_SHOW_STATS=1 "$bin" "$config" "$games" "$stride" "$1" 2>&1 >/dev/null | grep -E '^ *(malloc requested|normal|small|large|huge)' | head -6; }
a="$(stat 0)"; b="$(stat "$reps")"
if [[ -n "$a$b" ]]; then echo "mimalloc counters (reps 0 | reps $reps):"; paste -d'|' <(echo "$a") <(echo "$b"); else echo "mimalloc stats not compiled in: no allocation counts"; fi
