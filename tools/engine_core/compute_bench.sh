#!/usr/bin/env bash
# Reproduces the numbers of docs/ENGINE_COMPUTE.md 2 and 3 (build cache seeding, test profiles).
#
#   tools/engine_core/compute_bench.sh profiles WORK_DIR     # cold build + run + incremental per profile
#   tools/engine_core/compute_bench.sh seed WORK_DIR         # cold (warm dir) vs seeded builds
#
# Run from a scratch worktree (the script edits one source file for the incremental steps and restores it
# with git). Every step that can take minutes holds the heavy lock (/root/.ti4-heavy.lock) on its own.
# Env: JOBS (default 4, CARGO_BUILD_JOBS of all steps), PROFILES (default "gate lockstep lockstep0"),
#      PKG (default ti4-engine). All target dirs live in WORK_DIR and are removed again unless KEEP=1.
# Output: one `RESULT ...` line per step on stdout; cargo logs in WORK_DIR/*.log.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
mode="${1:?profiles|seed}"; work="${2:?WORK_DIR}"
jobs="${JOBS:-4}"; pkg="${PKG:-ti4-engine}"; profiles="${PROFILES:-gate lockstep lockstep0}"
mkdir -p "$work"; cd "$repo"
export CARGO_BUILD_JOBS="$jobs"
free_gb() { df -BG --output=avail /root/deploytmp | tail -1 | tr -dc 0-9; }
timed() { # LABEL LOGFILE CMD... : runs under the heavy lock, prints RESULT label seconds rc
  local label="$1" log="$2"; shift 2
  local t0 rc
  t0=$(date +%s.%N)
  flock /root/.ti4-heavy.lock "$@" > "$log" 2>&1; rc=$?
  printf 'RESULT %-34s %8.1f s rc=%s (free disk %s GB)\n' "$label" "$(echo "$(date +%s.%N) - $t0" | bc)" "$rc" "$(free_gb)"
}
poke() { printf '\n// compute_bench touch %s\n' "$RANDOM" >> "$1"; }
unpoke() { git checkout -q -- "$1"; }
leaf="crates/${PKG:-ti4-engine}/src/lib.rs"
if [[ "$mode" == profiles ]]; then
  for p in $profiles; do
    td="$work/t-$p"; export CARGO_TARGET_DIR="$td"; rm -rf "$td"
    [[ "$(free_gb)" -lt 3 ]] && { echo "RESULT abort: less than 3 GB free"; exit 1; }
    timed "$p cold build (no-run)" "$work/$p-build.log" cargo test --profile "$p" -p "$pkg" --lib --no-run
    timed "$p test run" "$work/$p-run.log" cargo test --profile "$p" -p "$pkg" --lib
    echo "RESULT $p tests: $(grep -E '^test result' "$work/$p-run.log" | tail -1)"
    poke "$leaf"
    timed "$p incremental build (1 file)" "$work/$p-incr.log" cargo test --profile "$p" -p "$pkg" --lib --no-run
    unpoke "$leaf"
    echo "RESULT $p target dir: $(du -sh "$td" | cut -f1)"
    [[ "${KEEP:-0}" == 1 ]] || rm -rf "$td"
  done
elif [[ "$mode" == seed ]]; then
  p="${PROFILE:-gate}"; warm="$work/warm"; rm -rf "$warm"
  timed "cold build into the warm dir" "$work/warm.log" "$here/target_dir.sh" warm "$warm" test --profile "$p" -p "$pkg" --lib --no-run
  echo "RESULT warm dir: $(du -sh "$warm" | cut -f1)"
  stamp="$work/stamp"; touch "$stamp"; sleep 1
  # Seeded build of the unchanged checkout. The scratch worktree IS a different path than the warm build
  # only when the warm dir was built elsewhere; here it is the same checkout, so the second part copies
  # the checkout to another path first.
  other="$work/checkout2"; rm -rf "$other"; git worktree add -q --detach "$other" HEAD 2>/dev/null || { echo "RESULT cannot create $other"; exit 1; }
  ( cd "$other"
    dest="$work/seeded-1"; export CARGO_TARGET_DIR="$dest"
    t0=$(date +%s.%N); tools/engine_core/target_dir.sh seed "$dest" "$warm" > "$work/seed1.log" 2>&1
    echo "RESULT seed (hard links + mtimes)          $(echo "$(date +%s.%N) - $t0" | bc) s: $(tail -2 "$work/seed1.log" | tr '\n' ' ')"
    echo "RESULT seeded dir apparent size $(du -sh --apparent-size "$dest" | cut -f1), extra disk $(du -sh "$dest" | cut -f1) (du counts links once per dir)"
    timed "seeded build, same commit (no-run)" "$work/seeded-1.log" cargo test --profile "$p" -p "$pkg" --lib --no-run
    echo "RESULT seeded same commit, crates compiled: $(grep -c '^ *Compiling' "$work/seeded-1.log") ($(grep '^ *Compiling' "$work/seeded-1.log" | awk '{print $2}' | tr '\n' ' '))"
    rm -rf "$dest"
    # one changed leaf file in the engine
    poke "$leaf"
    dest="$work/seeded-2"; export CARGO_TARGET_DIR="$dest"
    tools/engine_core/target_dir.sh seed "$dest" "$warm" > "$work/seed2.log" 2>&1
    timed "seeded build, 1 changed file (no-run)" "$work/seeded-2.log" cargo test --profile "$p" -p "$pkg" --lib --no-run
    echo "RESULT seeded 1 changed file, crates compiled: $(grep -c '^ *Compiling' "$work/seeded-2.log") ($(grep '^ *Compiling' "$work/seeded-2.log" | awk '{print $2}' | tr '\n' ' '))"
    unpoke "$leaf"; rm -rf "$dest"
    # control: unseeded cold build in the other checkout (same as the warm cold build)
  )
  echo "RESULT warm dir files changed by the seeded builds: $(find "$warm" -type f -newer "$stamp" ! -name '*.lock' ! -name '.warm*' | wc -l)"
  git worktree remove --force "$other" 2>/dev/null
  [[ "${KEEP:-0}" == 1 ]] || rm -rf "$warm"
fi
