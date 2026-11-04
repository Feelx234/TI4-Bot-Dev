#!/usr/bin/env bash
# D2-9 lane 9a: the artifact check (docs/todo/D2-9-strip.md 2.3). The graph check says what CAN link; this says what
# DID: the built server binary or the wasm must hold no legacy-engine env-var literal, no legacy module name and no
# panic-location path of the legacy crate directory.
#
#   tools/engine_core/check_output_probes.sh [--allow-legacy-probe] BINARY...    exit 1 on a hit
#   tools/engine_core/check_output_probes.sh --self-test                          synthetic files, no build
#
# --allow-legacy-probe  a hit is printed as WARN and the exit status stays 0 (a false positive the user accepted, e.g.
#                       a path that merely contains `crates/ti4-engine/src`); deploy.sh passes it through, default off.
# Exact patterns: `crates/ti4-engine/src/` with the trailing slash (the thin crate is `crates/ti4-engine-next/`).
set -uo pipefail
PROBES=(
  'TI4_BAIL_PROBE' 'TI4_DUAL_FAIL_FAST' 'TI4_AREAS' 'TI4_CHURN' 'TI4_LEND_PROOF'
  'ti4_engine_legacy' 'reverse_lend' 'lend_table' 'lend-string'
  'crates/ti4-engine/src/'
)

probe_file() {  # prints one line per hit; returns 1 on any hit
  local f="$1" p n bad=0
  [[ -r "$f" && -s "$f" ]] || { echo "ERROR $f: missing, unreadable or empty"; return 2; }
  for p in "${PROBES[@]}"; do
    n=$(grep -a -o -F -e "$p" -- "$f" | wc -l)
    if (( n > 0 )); then echo "HIT $f: '$p' x$n"; bad=1; fi
  done
  return $bad
}

self_test() {
  local tmp rc=0 p; tmp="$(mktemp -d "${TMPDIR:-/tmp}/d29a-probe.XXXXXX")" || return 1
  trap 'rm -rf "$tmp"' RETURN
  head -c 4096 /dev/urandom > "$tmp/clean"; printf 'panic at crates/ti4-engine-next/src/lib.rs and crates/ti4-engine-core/src/x.rs\0' >> "$tmp/clean"
  probe_file "$tmp/clean" >/dev/null || { echo "SELF-TEST FAILED: clean file flagged"; rc=1; }
  for p in "${PROBES[@]}"; do
    { head -c 512 /dev/urandom; printf '\0xx%s/yy\0' "$p"; } > "$tmp/dirty"
    probe_file "$tmp/dirty" >/dev/null && { echo "SELF-TEST FAILED: '$p' not detected"; rc=1; }
  done
  : > "$tmp/empty"; probe_file "$tmp/empty" >/dev/null && { echo "SELF-TEST FAILED: empty file passed"; rc=1; }
  "$0" "$tmp/clean" >/dev/null || { echo "SELF-TEST FAILED: CLI rejects the clean file"; rc=1; }
  "$0" "$tmp/dirty" >/dev/null && { echo "SELF-TEST FAILED: CLI accepts the dirty file"; rc=1; }
  "$0" --allow-legacy-probe "$tmp/dirty" >/dev/null || { echo "SELF-TEST FAILED: --allow-legacy-probe does not downgrade"; rc=1; }
  (( rc == 0 )) && echo "self-test ok: ${#PROBES[@]} probes each detected, clean and empty files handled, override works"
  return $rc
}

allow=0; files=()
for a in "$@"; do
  case "$a" in
    --self-test) self_test; exit $? ;;
    --allow-legacy-probe) allow=1 ;;
    -h|--help) sed -n '2,12p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) files+=("$a") ;;
  esac
done
(( ${#files[@]} )) || { echo "usage: $0 [--allow-legacy-probe] BINARY... | --self-test" >&2; exit 2; }
rc=0
for f in "${files[@]}"; do
  out="$(probe_file "$f")"; r=$?
  [[ -n "$out" ]] && echo "$out"
  if (( r == 2 )); then rc=1                       # a missing file is never an accepted false positive
  elif (( r == 1 )); then
    if (( allow )); then echo "WARN $f: legacy probe hits accepted by --allow-legacy-probe"; else rc=1; fi
  else echo "ok   $f: no legacy literal"; fi
done
exit $rc
