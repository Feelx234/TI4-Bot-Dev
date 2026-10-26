#!/usr/bin/env bash
# Profiling of the engine (docs/ENGINE_OPTIMIZATION.md). Every step is reproducible from here.
#
#   tools/engine_profile/profile.sh build            release builds with symbols: plain (timing) and
#                                                    frame pointers (perf call stacks), private target dirs
#   tools/engine_profile/profile.sh selfplay [CFG]   perf of bot-only games (default profile-4p.json, 1 worker)
#                                                    and the category breakdown (categorize.py)
#   tools/engine_profile/profile.sh views [CFG] [N]  bot games with the server's views at every question,
#                                                    the server replay and a bare replay (engine_profile game)
#   tools/engine_profile/profile.sh solo [SEED STEPS PLAYERS]   random seats, natively (ti4_run_seeded)
#   tools/engine_profile/profile.sh resume SAVE      the browser host's resume of an offline save, natively
#   tools/engine_profile/profile.sh wasm [SAVE]      wasm-release build; solo (js/run.mjs) and resume in Node
#   tools/engine_profile/profile.sh perf CMD...      perf record -g of any command + categorize.py
#   tools/engine_profile/profile.sh calls [CFG]      call counts of hot functions (uprobes, root)
#   tools/engine_profile/profile.sh ab BIN_A BIN_B [CFG] [REPEATS] [WORKERS]   CPU time A/B + hash check
#   tools/engine_profile/profile.sh verify BIN       every bench (bench, bench-v1..v5) with a binary
#
# Environment: EOP_TARGET (plain build, default /tmp/eop-target), EOP_TARGET_FP (frame pointers,
# default /tmp/eop-target-fp), EOP_OUT (perf data and results, default /tmp/eop-out). The target dirs
# are large (about 1 GB each); delete them when done. Load on the machine changes wall time, not the
# shares of a profile: compare CPU time (ab, verify) and repeat.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
plain="${EOP_TARGET:-/tmp/eop-target}"
fp="${EOP_TARGET_FP:-/tmp/eop-target-fp}"
out="${EOP_OUT:-/tmp/eop-out}"
mkdir -p "$out"
export SELFPLAY_RESULTS=off
symbols=(env CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=line-tables-only)

perf_categorize() {
  local name="$1"; shift
  perf record -F 499 -g -o "$out/$name.data" "$@"
  perf script -i "$out/$name.data" --show-mmap-events -F tid,ip,sym --no-inline 2>/dev/null > "$out/$name.txt"
  python3 -I "$here/categorize.py" "$out/$name.txt" --json "$out/$name.json" --top 40
}

cmd="${1:-help}"; [[ $# -gt 0 ]] && shift
case "$cmd" in
  build)
    (cd "$repo" && "${symbols[@]}" CARGO_TARGET_DIR="$plain" nice cargo build -q -p ti4-selfplay --release --bins --example engine_profile)
    (cd "$repo" && "${symbols[@]}" CARGO_TARGET_DIR="$fp" RUSTFLAGS="-C force-frame-pointers=yes" \
      nice cargo build -q -p ti4-selfplay --release --bins --example engine_profile)
    ;;
  selfplay)
    cfg="${1:-$here/profile-4p.json}"
    perf_categorize selfplay "$fp/release/selfplay" run --config "$cfg" --no-cache --workers 1 \
      --out "$out/selfplay.json" --quiet --no-table
    ;;
  views)
    "$plain/release/examples/engine_profile" game "${1:-$here/profile-4p.json}" "${2:-4}"
    ;;
  solo)
    "$plain/release/examples/engine_profile" solo "${1:-3}" "${2:-2000}" "${3:-8}"
    ;;
  resume)
    "$plain/release/examples/engine_profile" resume "${1:?a save or feedback report}" 3
    ;;
  wasm)
    (cd "$repo" && CARGO_TARGET_DIR="$plain" nice cargo build -q -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release)
    wasm="$plain/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm"
    node "$repo/crates/ti4-wasm/js/run.mjs" "$wasm" 3 2000 8
    [[ -n "${1:-}" ]] && node "$here/wasm_resume.mjs" "$wasm" "$1" 3
    ;;
  perf)
    perf_categorize custom "$@"
    ;;
  calls)
    "$here/count_calls.sh" "$fp/release/selfplay" -- "$fp/release/selfplay" run --config "${1:-$here/profile-4p.json}" \
      --no-cache --workers 1 --out "$out/calls.json" --quiet --no-table
    ;;
  ab)
    "$here/ab_time.sh" "$@"
    ;;
  verify)
    "$here/verify_benches.sh" "$@"
    ;;
  *)
    sed -n '2,24p' "${BASH_SOURCE[0]}"
    ;;
esac
