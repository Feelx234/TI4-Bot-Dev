#!/usr/bin/env bash
# Regenerates the size tables of docs/BUILD_SIZES_AND_TIMES.md from what is already built. It builds
# nothing (unless --build-wasm / --build-release are given) and reads, never writes, the target dir.
#
#   tools/build_report.sh                                  report on ./target (or $CARGO_TARGET_DIR)
#   tools/build_report.sh --target /path/to/target         another target dir
#   tools/build_report.sh --build-wasm                     first: cargo build -p ti4-wasm --profile wasm-release
#   tools/build_report.sh --build-release                  first: cargo build --release -p ti4-server --bin server -p ti4-selfplay
#   tools/build_report.sh --names                          also analyse with twiggy / cargo-bloat (needs the tools:
#                                                          cargo install twiggy cargo-bloat; builds unstripped copies in
#                                                          <target>-names, which is deleted unless --keep-names)
#   tools/build_report.sh --web                            also `vite build` of web/ (needs web/node_modules) and list dist/
#   tools/build_report.sh --out report.md                  write there (default: stdout)
#
# Timings are not part of it: see "How to reproduce" in docs/BUILD_SIZES_AND_TIMES.md (build_report/measure.sh).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
out=""; build_wasm=0; build_release=0; names=0; keep_names=0; web=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) target="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --build-wasm) build_wasm=1; shift ;;
    --build-release) build_release=1; shift ;;
    --names) names=1; shift ;;
    --keep-names) keep_names=1; shift ;;
    --web) web=1; shift ;;
    -h|--help) sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[[ -n "$out" ]] && exec > "$out"

export CARGO_TARGET_DIR="$target"
cd "$repo"

if (( build_wasm )); then
  cargo build -q -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release >&2
fi
if (( build_release )); then
  cargo build -q --release -p ti4-server --bin server -p ti4-selfplay >&2
fi

h() { printf '\n## %s\n\n' "$1"; }
fence() { printf '```\n'; cat; printf '```\n'; }

echo "# Build size report"
echo
echo "- date: $(TZ=Europe/Berlin date '+%F %T %Z')   commit: $(git rev-parse --short=8 HEAD)   target: $target"
echo "- machine: $(nproc) cores, load $(cut -d' ' -f1-3 /proc/loadavg), $(rustc -V)"
echo "- disk: $(df -h --output=avail "$target" 2>/dev/null | tail -1 | tr -d ' ') free where the target dir is"

h "Native binaries"
{
  printf '%-44s %12s %10s %10s %10s\n' file bytes text data bss
  for f in "$target"/release/* "$target"/debug/* "$target"/*/release/* "$target"/*/debug/*; do
    [[ -f "$f" && -x "$f" && "$f" != *.so && "$f" != *.d && "$f" != *.dwp ]] || continue
    [[ "$(head -c4 "$f" | tail -c3)" == "ELF" ]] || continue
    read -r t d b _ < <(size "$f" | tail -1)
    printf '%-44s %12s %10s %10s %10s\n' "${f#"$target"/}" "$(stat -c %s "$f")" "$t" "$d" "$b"
  done
  for f in "$target"/release/deps/libti4_*.so "$target"/debug/deps/libti4_*.so; do
    [[ -f "$f" ]] && printf '%-44s %12s  (shared library, from a cdylib)\n' "${f#"$target"/}" "$(stat -c %s "$f")"
  done
} | fence

h "WebAssembly (crates/ti4-wasm, profile wasm-release)"
wasm="$target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm"
if [[ -f "$wasm" ]]; then
  { ls -l "$wasm"; node scripts/wasm-size-gate.mjs "$wasm" 2>&1 || true; echo; cat crates/ti4-wasm/size-budget.json; } | fence
  shipped="web/src/offline/local/engine/ti4_wasm.wasm"
  if [[ -f "$shipped" ]]; then
    echo; echo "The file web/ ships (after wasm-opt, if it ran):"
    { ls -l "$shipped"; node scripts/wasm-size-gate.mjs "$shipped" 2>&1 || true; } | fence
  fi
else
  echo "(not built: cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release, or --build-wasm)"
fi

if (( web )); then
  h "Web bundle (vite build of web/)"
  ( cd web && TI4_WASM_FILE="$wasm" npx vite build >/dev/null 2>&1 && ls -lS dist/assets | awk 'NR>1{printf "%10s  %s\n",$5,$9}'; du -sh dist | awk '{print "total: "$1}' ) | fence
elif [[ -d web/dist/assets ]]; then
  h "Web bundle (existing web/dist)"
  { ls -lS web/dist/assets | awk 'NR>1{printf "%10s  %s\n",$5,$9}'; du -sh web/dist | awk '{print "total: "$1}'; } | fence
fi

h "Embedded content (ti4-content/content/*.json, compiled into every binary)"
{ du -cb crates/ti4-content/content/*.json | tail -1 | awk '{print "files: "$1" bytes"}'; ls -S crates/ti4-content/content/*.json | head -5 | xargs -I{} sh -c 'printf "%10s  %s\n" $(stat -c %s {}) {}'; } | fence

h "Target directory: where the disk goes"
python3 "$here/build_report/target_usage.py" "$target" 12 | fence

if (( names )); then
  nt="${target}-names"
  h "Symbols (unstripped copies built in $nt)"
  if command -v twiggy >/dev/null; then
    CARGO_TARGET_DIR="$nt" cargo build -q -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release \
      --config profile.wasm-release.strip=false --config profile.wasm-release.debug=false >&2
    w="$nt/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm"
    echo "### wasm by crate (twiggy top, grouped by first path segment)"
    twiggy top -n 1000000 --format csv "$w" > "$nt/twiggy.csv"
    python3 "$here/build_report/group_symbols.py" "$nt/twiggy.csv" 25 | fence
    echo "### wasm: biggest items"
    twiggy top -n 25 "$w" | fence
  else
    echo "(twiggy not installed: cargo install twiggy)"
  fi
  if command -v cargo-bloat >/dev/null; then
    export CARGO_PROFILE_RELEASE_STRIP=false CARGO_TARGET_DIR="$nt"
    for bin in server selfplay; do
      pkg=ti4-server; [[ $bin == selfplay ]] && pkg=ti4-selfplay
      echo "### $bin by crate (cargo bloat --crates)"
      cargo bloat --release -p "$pkg" --bin "$bin" --crates -n 25 2>/dev/null | fence
      echo "### $bin biggest functions"
      cargo bloat --release -p "$pkg" --bin "$bin" -n 25 2>/dev/null | fence
    done
  else
    echo "(cargo-bloat not installed: cargo install cargo-bloat)"
  fi
  (( keep_names )) || rm -rf "$nt"
fi
