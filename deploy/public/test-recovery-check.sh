#!/usr/bin/env bash
# Self-test of recovery-check.sh with a fake server: parse, compare and the preflight's handling of
# refused saves (docs/D28E_RESUME.md): a save the product refuses on purpose (an older rules
# version, Thunder's Edge content, a dropped faction) is counted apart from failures, needs no
# --allow-failed, and must name its reason. Run: deploy/public/test-recovery-check.sh
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CHECK="$HERE/recovery-check.sh"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

# --- parse ---------------------------------------------------------------------------------
cat >"$tmp/log" <<'L'
Durable storage: refused 'game_a1' (session replay): Game cannot be loaded: this game was saved with an older version of the rules (version 0); only games of version 36 or newer can be played, so it cannot be opened
Durable storage: could not recover 'game_b2' (session replay): Snapshot mismatch
Durable storage: refused 'game_c3' (session replay): Game cannot be loaded: Thunder's Edge content is switched off in this version
Durable storage: recovered 4 game(s); 1 failed; 2 refused (saved files retained)
L
[[ "$("$CHECK" parse "$tmp/log")" == "recovered=4 failed=1 refused=2 refused_ids=game_a1,game_c3 ids=game_b2" ]] || fail "parse"
echo 'Durable storage: recovered 3 game(s); 0 failed (saved files retained)' >"$tmp/old"
[[ "$("$CHECK" parse "$tmp/old")" == "recovered=3 failed=0 refused=0 refused_ids= ids=" ]] || fail "parse of an older server's line"
"$CHECK" compare "" "" || fail "compare of nothing"
"$CHECK" compare "game_b2" "game_b2" || fail "compare of an allowed failure"
if "$CHECK" compare "" "game_b2" >/dev/null; then fail "compare accepted an unexpected failure"; fi

# --- preflight against a fake server -----------------------------------------------------------
mkdata() { # $1 = number of game dirs
  rm -rf "$tmp/live"; mkdir -p "$tmp/live" "$tmp/web"
  for i in $(seq 1 "$1"); do mkdir "$tmp/live/game_$i"; done
}
mkserver() { # stdin = the lines the fake server prints
  { echo '#!/usr/bin/env bash'; echo "cat <<'L'"; cat; echo 'L'; echo 'sleep 60'; } >"$tmp/server"
  chmod +x "$tmp/server"
}
preflight() { "$CHECK" preflight --bin "$tmp/server" --web-dir "$tmp/web" --data-dir "$tmp/live" "$@"; }

# Every alpha save refused for its version: the preflight passes and says so.
mkdata 3
mkserver <<'L'
Durable storage: refused 'game_1' (session replay): Game cannot be loaded: this game was saved with an older version of the rules (version 0)
Durable storage: refused 'game_2' (session replay): Game cannot be loaded: this game was saved with an older version of the rules (version 0)
Durable storage: refused 'game_3' (session replay): Game cannot be loaded: this game was saved with an older version of the rules (version 0)
Durable storage: recovered 0 game(s); 0 failed; 3 refused (saved files retained)
L
out="$(preflight)" || fail "refusals should pass: $out"
grep -q 'REFUSED_IDS game_1 game_2 game_3' <<<"$out" || fail "refused ids: $out"
grep -q '3 game(s) refused cleanly' <<<"$out" || fail "refused count: $out"
grep -q 'preflight OK: 0 recovered, 3 refused cleanly, 0 allowed' <<<"$out" || fail "verdict: $out"

# A damaged save is still a failure.
mkdata 2
mkserver <<'L'
Durable storage: refused 'game_1' (session replay): Game cannot be loaded: this game was saved with an older version of the rules (version 0)
Durable storage: could not recover 'game_2' (session replay): Snapshot mismatch
Durable storage: recovered 0 game(s); 1 failed; 1 refused (saved files retained)
L
if preflight >/dev/null 2>&1; then fail "a damaged save passed"; fi
preflight --allow-failed game_2 >/dev/null || fail "an allowed failure should pass"

# A refusal without a reason is not clean.
mkdata 1
mkserver <<'L'
Durable storage: refused 'game_1' (session replay): boom
Durable storage: recovered 0 game(s); 0 failed; 1 refused (saved files retained)
L
if preflight >/dev/null 2>&1; then fail "a refusal with no reason passed"; fi

# The counts must add up to the game directories.
mkdata 3
mkserver <<'L'
Durable storage: recovered 1 game(s); 0 failed; 1 refused (saved files retained)
L
if preflight >/dev/null 2>&1; then fail "a lost game passed"; fi
echo "recovery-check self-test: ok"
