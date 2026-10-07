#!/usr/bin/env bash
# Checks run_game.sh's preset choice with a stub in place of the real playthrough.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/tools/nightly-smoke"
cp "$here/config.sh" "$here/run_game.sh" "$tmp/tools/nightly-smoke/"
printf '#!/usr/bin/env bash\nexit 0\n' > "$tmp/tools/nightly-smoke/_run_inner.sh"
chmod +x "$tmp/tools/nightly-smoke/"*.sh
# A repo to record the commit from (the run's meta.json says which code it played).
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
git -C "$tmp" init -q -b main && echo hi > "$tmp/README" && git -C "$tmp" add README && git -C "$tmp" commit -q -m init

# The first checks pin one preset and no rotation; the list checks below set their own.
start() { # start <probability> <run-dir>
  NIGHTLY_REPO="$tmp" NIGHTLY_PRESET_PROBABILITY="$1" NIGHTLY_PRESET=combat NIGHTLY_PRESET_ROTATE_PERCENT=0 \
    "$tmp/tools/nightly-smoke/run_game.sh" start "$2" > /dev/null
}

fail() { echo "FAIL: $*" >&2; exit 1; }

start 100 "$tmp/always"
grep -q '"preset": "combat"' "$tmp/always/meta.json" || fail "100% should pick the combat preset"
grep -q 'TI4_SMOKE=1 TI4_SMOKE_PRESET=combat ' "$tmp/always/meta.json" || fail "repro should carry the preset"
python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$tmp/always/meta.json" || fail "meta.json must stay valid JSON"

head_commit=$(git -C "$tmp" rev-parse HEAD)
grep -q "\"commit\": \"$head_commit\"" "$tmp/always/meta.json" || fail "meta.json should record the commit under test"
python3 -c "import json,sys; m=json.load(open(sys.argv[1])); assert isinstance(m['dirty_files'], int) and m['dirty_files'] >= 1" "$tmp/always/meta.json" \
  || fail "meta.json should count the files that differ from the commit (the copied tools dir is untracked here)"

start 0 "$tmp/never"
grep -q '"preset": ""' "$tmp/never/meta.json" || fail "0% should pick no preset"
grep -q 'TI4_SMOKE_PRESET' "$tmp/never/meta.json" && fail "repro must not mention a preset when none was picked"
python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$tmp/never/meta.json" || fail "meta.json must stay valid JSON"

hits=0
for i in $(seq 1 60); do
  start 50 "$tmp/half$i"
  grep -q '"preset": "combat"' "$tmp/half$i/meta.json" && hits=$((hits + 1))
done
[ "$hits" -ge 15 ] && [ "$hits" -le 45 ] || fail "50% picked the preset $hits/60 times"
echo "ok: preset pick (50% picked $hits/60)"

# A list picks each member over many runs, and the rotation suffix goes on about as often as asked.
declare -A picked
rot=0
for i in $(seq 1 80); do
  NIGHTLY_REPO="$tmp" NIGHTLY_PRESET_PROBABILITY=100 NIGHTLY_PRESET="cards agenda relics" NIGHTLY_PRESET_ROTATE_PERCENT=50 \
    "$tmp/tools/nightly-smoke/run_game.sh" start "$tmp/list$i" > /dev/null
  name=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['preset'])" "$tmp/list$i/meta.json")
  base=${name%+rot}
  picked[$base]=1
  case "$name" in *+rot) rot=$((rot + 1)) ;; esac
  grep -q "TI4_SMOKE_PRESET=$name " "$tmp/list$i/meta.json" || fail "repro should carry $name"
done
[ "${#picked[@]}" -eq 3 ] || fail "the list should reach all three presets, reached ${!picked[*]}"
[ "$rot" -ge 20 ] && [ "$rot" -le 60 ] || fail "50% rotation was added $rot/80 times"
# leaders always rotates inside the preset, so it never gets the suffix.
for i in $(seq 1 10); do
  NIGHTLY_REPO="$tmp" NIGHTLY_PRESET_PROBABILITY=100 NIGHTLY_PRESET=leaders NIGHTLY_PRESET_ROTATE_PERCENT=100 \
    "$tmp/tools/nightly-smoke/run_game.sh" start "$tmp/lead$i" > /dev/null
  grep -q '"preset": "leaders"' "$tmp/lead$i/meta.json" || fail "leaders must not get +rot"
done
echo "ok: preset list and rotation (+rot $rot/80)"
