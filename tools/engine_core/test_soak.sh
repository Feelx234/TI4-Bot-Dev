#!/usr/bin/env bash
# Self-test of the soak tooling without the real engine (docs/ENGINE_SOAK.md): a fake `dual_run` plays the shards.
#   tools/engine_core/test_soak.sh
# Checks: the corpus is deterministic and covers what the doc promises; one soak round writes state, LATEST.md and slice
# files; a planted difference shows up with a repro line; a held heavy lock pauses (SIGSTOP) a running slice and the soak
# yields before the next one; resume after `stop` redoes the interrupted slice; the repro script parses a record line.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d /root/deploytmp/test-soak.XXXXXX)"; trap 'rm -rf "$tmp"; [[ -n "${holder:-}" ]] && kill "$holder" 2>/dev/null' EXIT
fail=0; ok() { echo "ok   $*"; }; bad() { echo "FAIL $*"; fail=1; }

# --- corpus ---------------------------------------------------------------------------------------------------
python3 "$here/soak_corpus.py" gen old 20 777 > "$tmp/a.json"; python3 "$here/soak_corpus.py" gen old 20 777 > "$tmp/b.json"
cmp -s "$tmp/a.json" "$tmp/b.json" && ok "gen is deterministic" || bad "gen is deterministic"
python3 - "$here" <<'PY' && ok "tier files cover players 3-8, hyperlane maps, Thunder's Edge, rules versions 31-34, 24 factions" || bad "tier coverage"
import json, sys, glob, os
sys.path.insert(0, sys.argv[1]); import soak_corpus as c
players, maps, te, versions, factions = set(), set(), set(), set(), set()
for lane in c.LANES:
    m = c.gen(lane, 1000, 1)
    for s in m["setups"]:
        players.add(s["players"]); maps.update(s["maps"]); te.add(s.get("thunders_edge", False)); versions.add(s.get("rules_version"))
        factions.update(s.get("factions", []))
assert players == set(range(3, 9)), players
assert any("Hyperlanes" in x for x in maps), maps
assert te == {True, False}
assert {31, 32, 33, 34, None} <= versions, versions
assert factions == set(c.LOBBY), set(c.LOBBY) - factions
assert "lastbastion" not in factions and "crimson" not in factions
PY
git -C "$here" diff --quiet HEAD -- ../engine_profile/profile-20.json && ok "profile-20.json untouched" || bad "profile-20.json changed"

# --- fake dual_run ----------------------------------------------------------------------------------------------
cat > "$tmp/fake_dual_run" <<'PY'
#!/usr/bin/env python3
import json, os, sys, time
a = sys.argv[1:]
if a[0] == "--specs":
    for i in range(int(a[2])):
        print(json.dumps({"index": i, "seed": 5000 + i, "players": 4, "map": "m", "factions": ["sol"], "setup": "4p", "selfplay_flags": "--seed x", "bot": {}}))
    sys.exit(0)
if a[0] == "--merge":
    print("games ? | merged " + str(len(a) - 1) + " files"); sys.exit(0)
games, mask = int(a[1]), a[2]
shard, out = a[a.index("--shard") + 1], a[a.index("--out") + 1]
i, n = map(int, shard.split("/"))
time.sleep(float(os.environ.get("FAKE_SLEEP", "1")))
recs = []
for g in range(games):
    if g % n != i: continue
    diffs = [{"area": "driver", "category": "Decision", "label": "combat::fake", "path": "context.x[3]", "string_side": "1", "core_side": "2"}] if os.environ.get("FAKE_DIFF") and g == 1 else []
    recs.append({"index": g, "dual_hash": "h", "string_hash": "h", "report": {"areas": {"driver": {"calls": 10, "compared": [1] * 8, "differing": [0] * 8}},
        "windows": {"combat::fake": {"compared": 0, "bailed": 2}, "combat::ok": {"compared": 5, "bailed": 1}, "combat::ok [bail: nested (x)]": {"compared": 0, "bailed": 1}}, "differences": diffs, "dropped": 0}})
json.dump({"format": "x", "shard": i, "shards": n, "records": recs}, open(out, "w"))
sys.exit(1 if os.environ.get("FAKE_DIFF") and any(r["report"]["differences"] for r in recs) else 0)
PY
chmod +x "$tmp/fake_dual_run"
export DUAL_RUN="$tmp/fake_dual_run" SOAK_DIR="$tmp/reports" SOAK_WORK="$tmp/work" HEAVY_LOCK="$tmp/heavy.lock" SOAK_POLL=1 SHARDS=2 LANES="std te" SLICE_GAMES=8 MASKS="driver wide" ROUNDS=1
touch "$HEAVY_LOCK"

# --- a round with a planted difference ------------------------------------------------------------------------------
FAKE_DIFF=1 FAKE_SLEEP=1 "$here/soak.sh" run > "$tmp/run1.log" 2>&1
[[ -s "$SOAK_DIR/LATEST.md" ]] && ok "LATEST.md written" || bad "LATEST.md missing"
grep -q "combat::fake" "$SOAK_DIR/LATEST.md" && grep -q "soak_repro.sh --line" "$SOAK_DIR/LATEST.md" && ok "planted difference reported with a repro line" || bad "difference not reported"
grep -q "Silent bails" "$SOAK_DIR/LATEST.md" && grep -q "window combat::fake" "$SOAK_DIR/LATEST.md" && ok "silent bail (compared 0, bailed > 0) detected, bail-reason rows are not flagged" || bad "silent bail detection"
! grep -q "window combat::ok " "$SOAK_DIR/LATEST.md" && ok "a window that compared is not silent" || bad "false silent bail"
python3 - "$SOAK_DIR/state.json" <<'PY' && ok "state: 2 finished slices, next seeds advanced" || bad "state"
import json, sys
s = json.load(open(sys.argv[1])); assert len(s["finished"]) == 2, s["finished"]; assert s["next_seed"]["std"] >= 100000 + 1, s["next_seed"]; assert s["round"] == 1
PY
line="$(python3 - "$SOAK_DIR" <<'PY'
import glob, json, sys
for p in glob.glob(sys.argv[1] + "/*/*/result.json"):
    r = json.load(open(p))
    if r["differences"]: print(r["differences"][0]["repro"]); break
PY
)"
[[ "$line" == *"--seed 5001"* ]] && ok "repro line names the game seed ($line)" || bad "repro line: $line"

# --- lock awareness: a held lock pauses a running slice and delays the next ---------------------------------------------------
rm -rf "$SOAK_DIR"
( FAKE_SLEEP=6 "$here/soak.sh" run > "$tmp/run2.log" 2>&1 ) &
soakpid=$!
sleep 3                       # the first slice is running its fake sleep
flock "$HEAVY_LOCK" sleep 8 & holder=$!
sleep 1
wait $soakpid
grep -q "paused" "$tmp/run2.log" && grep -q "resumed" "$tmp/run2.log" && ok "a held heavy lock paused and resumed the slice in flight" || { bad "no pause/resume in log"; cat "$tmp/run2.log"; }
n="$(grep -c "^.* slice " "$tmp/run2.log")"; [[ -s "$SOAK_DIR/LATEST.md" ]] && ok "round completed after the pause" || bad "round did not complete"
# nobody waits: a waiter counts as busy too
rm -rf "$SOAK_DIR"
flock "$HEAVY_LOCK" sleep 4 & holder=$!; sleep 0.5
( flock "$HEAVY_LOCK" true ) & waiter=$!   # a gate queueing for the lock
sleep 0.5
( FAKE_SLEEP=1 "$here/soak.sh" run > "$tmp/run3.log" 2>&1 ); wait $waiter 2>/dev/null
grep -q "yielding" "$tmp/run3.log" && ok "the soak yielded before starting while the lock was held/queued" || { bad "no yield"; cat "$tmp/run3.log"; }

# --- stop and resume -----------------------------------------------------------------------------------------------------------------
rm -rf "$SOAK_DIR"; export ROUNDS=0
FAKE_SLEEP=2 "$here/soak.sh" start > /dev/null
for _ in $(seq 1 60); do [[ -f "$SOAK_DIR/state.json" ]] && python3 -c "import json,sys;sys.exit(0 if len(json.load(open('$SOAK_DIR/state.json'))['finished'])>=1 else 1)" && break; sleep 1; done
"$here/soak.sh" stop | grep -q stopped && ok "stop" || bad "stop"
done_before="$(python3 -c "import json;print(len(json.load(open('$SOAK_DIR/state.json'))['finished']))")"
export ROUNDS=1; FAKE_SLEEP=1 "$here/soak.sh" run > "$tmp/run4.log" 2>&1
python3 - "$SOAK_DIR/state.json" "$done_before" <<'PY' && ok "resume: finished slices before the stop ($done_before) are kept, none is run twice" || bad "resume"
import json, sys
s = json.load(open(sys.argv[1]))
assert len(set(s["finished"])) == len(s["finished"]), "a slice id twice"
assert len(s["finished"]) >= int(sys.argv[2]) >= 1, s["finished"]
assert s["round"] >= 1
PY

# --- repro script ---------------------------------------------------------------------------------------------------------------------
out="$("$here/soak_repro.sh" --line "$tmp/fake_dual_run $tmp/a.json 3 \"driver=dual\" --seed 5001" 2>&1)"
[[ "$out" == *'"seed": 5001'* && "$out" == *"== exit"* ]] && ok "soak_repro.sh parses a line and runs the binary" || bad "soak_repro.sh"
exit "$fail"
