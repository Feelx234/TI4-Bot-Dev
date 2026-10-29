#!/usr/bin/env bash
# Correctness test of the sharded dual run (tools/engine_core/dual_sharded.sh, `dual_run --shard/--merge`).
#
#   tools/engine_core/test_shards.sh DUAL_RUN [OUT_DIR]
#
# Env: GAMES (default 6, the first games of the profile), MASK (default agenda=dual, a cheap area),
#      SHARDS (default 3; 2 when the load average is above 8), CONFIG (default tools/engine_profile/profile-20.json).
#
# 1. the unsharded single process (`dual_run CONFIG GAMES MASK`, the reference) and the sharded run merged
#    must print the same text (only the lend nanoseconds, a clock reading, are masked) and exit the same;
# 2. mutations of the shard files must be refused: a dropped shard, a shard given twice, a game copied into
#    another shard, a game duplicated inside its shard, a dropped game, a shard of another run (other games
#    count) -- exit 2 `MERGE REFUSED`; and a changed string hash must come out as `HASH CHANGED game k`,
#    `hashes equal: false`, exit 1 (the verdict is per game).
# Exit 0 when everything holds.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
bin="${1:?DUAL_RUN}"; out="${2:-$(mktemp -d)}"
games="${GAMES:-6}"; mask="${MASK:-agenda=dual}"; config="${CONFIG:-tools/engine_profile/profile-20.json}"
shards="${SHARDS:-}"
if [[ -z "$shards" ]]; then
  load="$(cut -d' ' -f1 /proc/loadavg | cut -d. -f1)"; shards=3; (( load > 8 )) && shards=2
fi
mkdir -p "$out"; cd "$repo"
fail=0
ok() { echo "ok   $*"; }
bad() { echo "FAIL $*"; fail=1; }
norm() { sed -E 's/[0-9]+ ns\)/N ns)/' "$1"; }

# 1. equivalence
t0=$SECONDS; "$bin" "$config" "$games" "$mask" > "$out/ref.txt" 2> "$out/ref.err"; ref_rc=$?; t_ref=$((SECONDS - t0))
t0=$SECONDS; SHARDS="$shards" "$here/dual_sharded.sh" "$bin" "$config" "$games" "$mask" "$out/shards" > "$out/merged.txt" 2> "$out/merged.err"; merged_rc=$?; t_shard=$((SECONDS - t0))
echo "     unsharded ${t_ref} s (exit $ref_rc), $shards shards ${t_shard} s (exit $merged_rc), $games games, $mask"
if [[ $ref_rc == "$merged_rc" ]]; then ok "exit status equal ($ref_rc)"; else bad "exit status: unsharded $ref_rc, merged $merged_rc"; fi
if diff <(norm "$out/ref.txt") <(norm "$out/merged.txt") > "$out/diff.txt"; then ok "merged report identical to the unsharded one ($(wc -l < "$out/ref.txt") lines)"
else bad "merged report differs from the unsharded one (see $out/diff.txt)"; head -20 "$out/diff.txt"; fi
grep -q '^dual: [0-9]* calls, [0-9]* comparisons, [0-9]* differences$' "$out/merged.txt" && grep -q "^games $games | areas on dual: .* | hashes equal: true$" "$out/merged.txt" \
  && ok "summary lines present" || bad "summary lines missing in the merged output"

# 2. mutations
files=("$out"/shards/shard-*.json)
n=${#files[@]}
refused() { # NAME EXPECTED_TEXT FILES...
  local name="$1" want="$2"; shift 2
  "$bin" --merge "$@" > "$out/mut.txt" 2>&1; local rc=$?
  if [[ $rc == 2 ]] && grep -q "MERGE REFUSED.*$want" "$out/mut.txt"; then ok "refused: $name"; else bad "NOT refused as expected: $name (exit $rc: $(head -c 200 "$out/mut.txt"))"; fi
}
mut() { python3 -I - "$@" <<'PY'
import json, sys
src, dst, kind = sys.argv[1:4]
extra = sys.argv[4] if len(sys.argv) > 4 else None
d = json.load(open(src))
r = d["records"]
if kind == "dup_game_within":    r.append(json.loads(json.dumps(r[0])))
elif kind == "drop_game":        r.pop()
elif kind == "foreign_game":     r.append(json.load(open(extra))["records"][0])
elif kind == "other_games":      d["games"] += 1
elif kind == "string_hash":      r[0]["string_hash"] = "mutated"
elif kind == "plant_difference":
    r[0]["report"]["differences"].append({"area": "agenda", "category": "Value", "label": "planted", "path": "", "string_side": "a", "core_side": "b"})
json.dump(d, open(dst, "w"))
PY
}
# the files without one shard
for ((k = 0; k < n; k++)); do
  rest=(); for ((j = 0; j < n; j++)); do (( j != k )) && rest+=("${files[$j]}"); done
  refused "shard $k dropped" "shards missing: \[$k\]" "${rest[@]}"
done
refused "shard 0 given twice" "given twice" "${files[@]}" "${files[0]}"
mut "${files[1]}" "$out/m-foreign.json" foreign_game "${files[0]}"
refused "a game of shard 0 copied into shard 1" "belongs to shard" "${files[0]}" "$out/m-foreign.json" "${files[@]:2}"
mut "${files[0]}" "$out/m-dup.json" dup_game_within
refused "a game duplicated inside its shard" "appears twice" "$out/m-dup.json" "${files[@]:1}"
mut "${files[0]}" "$out/m-drop.json" drop_game
refused "a game dropped" "games missing" "$out/m-drop.json" "${files[@]:1}"
mut "${files[0]}" "$out/m-other.json" other_games
refused "a shard of another run (games count)" "does not belong" "$out/m-other.json" "${files[@]:1}"
# verdicts that must get through the merge
mut "${files[0]}" "$out/m-hash.json" string_hash
"$bin" --merge "$out/m-hash.json" "${files[@]:1}" > "$out/mut.txt" 2>&1; rc=$?
if [[ $rc == 1 ]] && grep -q '^HASH CHANGED game 0: dual .* string mutated$' "$out/mut.txt" && grep -q 'hashes equal: false' "$out/mut.txt"; then ok "a changed hash is reported per game (exit 1)"; else bad "changed hash not reported (exit $rc)"; fi
mut "${files[0]}" "$out/m-diff.json" plant_difference
"$bin" --merge "$out/m-diff.json" "${files[@]:1}" > "$out/mut.txt" 2>&1; rc=$?
if [[ $rc == 1 ]] && grep -q ' 1 differences$' "$out/mut.txt" && grep -q 'DUAL DIFFERENCE \[agenda/Value\] planted' "$out/mut.txt"; then ok "a planted difference comes through the merge (exit 1)"; else bad "planted difference lost (exit $rc)"; fi
# a failing shard is named
cp "${files[0]}" "$out/keep0.json"
( shim="$out/shim.sh"; printf '#!/usr/bin/env bash\n[[ "$*" == *"--shard 1/"* ]] && { echo boom >&2; exit 101; }\nexec %q "$@"\n' "$bin" > "$shim"; chmod +x "$shim"
  SHARDS="$shards" "$here/dual_sharded.sh" "$shim" "$config" "$games" "$mask" "$out/shards-fail" > "$out/fail.txt" 2>&1; echo $? > "$out/fail.rc" )
if [[ "$(cat "$out/fail.rc")" == 1 ]] && grep -q "^SHARD 1/$shards FAILED (exit 101" "$out/fail.txt" && grep -q 'MERGE REFUSED.*shards missing: \[1\]' "$out/fail.txt"; then ok "a failing shard fails the step and is named"; else bad "failing shard not reported"; cat "$out/fail.txt" | head; fi

[[ $fail == 0 ]] && echo "SHARDS TEST OK" || echo "SHARDS TEST FAILED"
exit $fail
