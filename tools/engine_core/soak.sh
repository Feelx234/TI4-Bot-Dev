#!/usr/bin/env bash
# The soak: corpus slices through the dual run, in the background, at low priority, politely (docs/ENGINE_SOAK.md).
#
#   tools/engine_core/soak.sh start     start it in the background (refuses a second one); logs to $SOAK_DIR/soak.log
#   tools/engine_core/soak.sh stop      stop it (its slice in flight is abandoned and redone on the next start)
#   tools/engine_core/soak.sh status    is it running; the head of LATEST.md; the log tail
#   tools/engine_core/soak.sh run       the same in the foreground (what `start` launches)
#   tools/engine_core/soak.sh report    rewrite LATEST.md from the state file
#
# Env (defaults): DUAL_RUN (required: a built dual_run, gate or release profile), SHARDS=3 (CPU share: this many busy
#   processes), SLICE_GAMES=40 (games per round, split over the lanes by weight), LANES (all of `soak_corpus.py lanes`),
#   MASKS="all driver wide" (rotated over lanes and rounds; a name or a raw `area=dual,...` spec), ROUNDS=0 (0: until
#   stopped), SEED_BASE=100000 (first seed of every lane; the corpus tiers use seeds from 1), SOAK_DIR=/root/soak-reports,
#   SOAK_WORK=/root/deploytmp/soak-work, SLICE_TIMEOUT=3600 (active seconds a slice may take), HEAVY_LOCK=/root/.ti4-heavy.lock.
#
# Lock awareness: the soak never holds the heavy lock. It runs a slice only while nobody holds the lock and nobody waits for
# it (/proc/locks), and while a slice runs it polls every 10 s: when a gate takes or queues for the lock, the slice's process
# group is SIGSTOPped (no CPU, memory stays) and continued when the lock is free and unqueued again. Slices run under
# `nice -n 19` and `ionice -c3`. No cron job; nothing starts it but this command.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cmd="${1:-}"
SOAK_DIR="${SOAK_DIR:-/root/soak-reports}"
SOAK_WORK="${SOAK_WORK:-/root/deploytmp/soak-work}"
HEAVY_LOCK="${HEAVY_LOCK:-/root/.ti4-heavy.lock}"
SHARDS="${SHARDS:-3}"
SLICE_GAMES="${SLICE_GAMES:-40}"
MASKS="${MASKS:-all driver wide}"
ROUNDS="${ROUNDS:-0}"
SEED_BASE="${SEED_BASE:-100000}"
SLICE_TIMEOUT="${SLICE_TIMEOUT:-3600}"
LANES="${LANES:-$(python3 -I "$here/soak_corpus.py" lanes | awk '{print $1}' | tr '\n' ' ')}"
state="$SOAK_DIR/state.json"
pidfile="$SOAK_DIR/soak.pid"
log="$SOAK_DIR/soak.log"

mask_spec() {
  case "$1" in
    all) echo "all=dual" ;;
    driver) echo "driver=dual" ;;
    wide) echo "driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual" ;;
    *=*) echo "$1" ;;
    *) echo "unknown mask name $1" >&2; return 1 ;;
  esac
}

say() { echo "$(date '+%F %T') $*"; }

gate_busy() {
  # Held: a non-blocking lock attempt fails. Queued: /proc/locks shows a waiter ("->") on the lock file's inode.
  flock -n "$HEAVY_LOCK" true 2>/dev/null || return 0
  local ino; ino="$(stat -c %i "$HEAVY_LOCK" 2>/dev/null)" || return 1
  grep -E -- "->.*:${ino} " /proc/locks >/dev/null 2>&1
}

wait_for_gate() {
  local said=0
  while gate_busy; do
    ((said)) || { say "yielding: a gate holds or waits for $HEAVY_LOCK"; said=1; }
    sleep 30
  done
  ((said)) && say "lock free, continuing"
  return 0
}

running_pid() { [[ -f "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null && cat "$pidfile"; }

run_slice() {  # lane first games maskname
  local lane="$1" first="$2" games="$3" mname="$4" mask id day dir pid active=0 paused=0 t0
  mask="$(mask_spec "$mname")" || return 1
  id="${lane}_${first}_${games}_${mname}"
  day="$(date +%F)"; dir="$SOAK_DIR/$day/$id"
  rm -rf "$dir"; mkdir -p "$dir/shards"
  python3 -I "$here/soak_corpus.py" gen "$lane" "$games" "$first" --out "$dir/config.json" || return 1
  "$DUAL_RUN" --specs "$dir/config.json" "$games" > "$dir/specs.jsonl" || { say "specs failed for $id"; return 1; }
  wait_for_gate
  t0=$SECONDS
  say "slice $id: $games games, mask $mask, $SHARDS shards"
  SHARDS="$SHARDS" nice -n 19 ionice -c3 "$here/dual_sharded.sh" "$DUAL_RUN" "$dir/config.json" "$games" "$mask" "$dir/shards" > "$dir/merged.txt" 2>&1 9>&- &
  pid=$!
  current_pid=$pid
  while kill -0 "$pid" 2>/dev/null; do
    if gate_busy; then
      kill -STOP -- "-$pid" 2>/dev/null; say "paused $id (gate on $HEAVY_LOCK)"
      while gate_busy; do sleep 15; done
      kill -CONT -- "-$pid" 2>/dev/null; say "resumed $id"
    fi
    sleep 10
    active=$((active + 10))
    if ((active > SLICE_TIMEOUT)); then
      say "slice $id exceeded $SLICE_TIMEOUT s of active time, killed"; kill -TERM -- "-$pid" 2>/dev/null; sleep 2; kill -KILL -- "-$pid" 2>/dev/null; break
    fi
  done
  wait "$pid"; local rc=$?
  current_pid=""
  local secs=$((SECONDS - t0))
  python3 -I - "$dir" "$id" "$lane" "$first" "$games" "$mname" "$mask" "$secs" "$rc" "$DUAL_RUN" <<'PY'
import json, sys
d, id_, lane, first, games, mname, mask, secs, rc, binary = sys.argv[1:]
json.dump({"id": id_, "lane": lane, "first_seed": int(first), "games": int(games), "mask_name": mname, "mask": mask,
           "seconds": int(secs), "exit": int(rc), "config": d + "/config.json", "dual_run": binary}, open(d + "/meta.json", "w"))
PY
  python3 -I "$here/soak_analyze.py" slice "$dir" "$state" "$SOAK_DIR" | tee -a "$SOAK_DIR/$day/slices.log"
  python3 -I "$here/soak_analyze.py" report "$state" "$SOAK_DIR/LATEST.md"
  # Keep the shard files only when they are evidence (a difference, a hash change, an incomplete slice).
  if python3 -I - "$dir/result.json" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
sys.exit(0 if (not r["differences"] and not r["hash_changes"] and not r["failed_shards"] and not r["merge_refused"]
               and r["games_played"] == r["games_planned"] and not r["dropped"]) else 1)
PY
  then rm -rf "$dir/shards"; fi
}

on_term() {
  say "stopping"
  if [[ -n "${current_pid:-}" ]]; then
    kill -CONT -- "-$current_pid" 2>/dev/null; kill -TERM -- "-$current_pid" 2>/dev/null; sleep 2; kill -KILL -- "-$current_pid" 2>/dev/null
  fi
  rm -f "$pidfile"; exit 0
}

case "$cmd" in
  run)
    : "${DUAL_RUN:?set DUAL_RUN to a built dual_run}"
    [[ -x "$DUAL_RUN" ]] || { echo "DUAL_RUN not executable: $DUAL_RUN" >&2; exit 2; }
    mkdir -p "$SOAK_DIR" "$SOAK_WORK"
    exec 9>"$SOAK_DIR/soak.lock"; flock -n 9 || { echo "a soak is already running (soak.lock)" >&2; exit 1; }
    set -m
    current_pid=""
    trap on_term TERM INT
    echo $$ > "$pidfile"
    repo="$(cd "$here/../.." && pwd)"
    python3 -I "$here/soak_analyze.py" meta "$state" started "$(date '+%F %T')"
    python3 -I "$here/soak_analyze.py" meta "$state" commit "$(git -C "$repo" rev-parse --short HEAD 2>/dev/null)"
    python3 -I "$here/soak_analyze.py" meta "$state" dual_run "$DUAL_RUN"
    python3 -I "$here/soak_analyze.py" meta "$state" shards "$SHARDS"
    python3 -I "$here/soak_analyze.py" meta "$state" rules_version "$(grep -o 'pub const RULES_VERSION: u32 = [0-9]*' "$repo/crates/ti4-model/src/state.rs" | grep -o '[0-9]*$')"
    say "soak start: lanes [$LANES] masks [$MASKS] shards $SHARDS slice_games $SLICE_GAMES rounds $ROUNDS binary $DUAL_RUN"
    round=0
    while :; do
      wait_for_gate
      while read -r lane first games mname; do
        run_slice "$lane" "$first" "$games" "$mname" || say "slice $lane/$first failed to run"
      done < <(python3 -I "$here/soak_analyze.py" plan "$state" "$LANES" "$SLICE_GAMES" "$SEED_BASE" "$MASKS")
      python3 -I "$here/soak_analyze.py" round "$state"
      round=$((round + 1))
      ((ROUNDS > 0 && round >= ROUNDS)) && break
    done
    say "soak done after $round rounds"
    rm -f "$pidfile"
    ;;
  start)
    mkdir -p "$SOAK_DIR"
    if p="$(running_pid)"; then echo "already running (pid $p)"; exit 1; fi
    : "${DUAL_RUN:?set DUAL_RUN to a built dual_run}"
    DUAL_RUN="$DUAL_RUN" setsid nohup "$0" run >> "$log" 2>&1 < /dev/null &
    sleep 2
    echo "started (pid $(cat "$pidfile" 2>/dev/null || echo '?')); log $log; summary $SOAK_DIR/LATEST.md"
    ;;
  stop)
    if p="$(running_pid)"; then
      kill -TERM "$p"; for _ in $(seq 1 20); do kill -0 "$p" 2>/dev/null || break; sleep 1; done
      if kill -0 "$p" 2>/dev/null; then kill -KILL -- "-$p" 2>/dev/null; kill -KILL "$p" 2>/dev/null; fi
      rm -f "$pidfile"; echo "stopped"
    else echo "not running"; fi
    ;;
  status)
    if p="$(running_pid)"; then echo "running (pid $p)"; else echo "not running"; fi
    [[ -f "$SOAK_DIR/LATEST.md" ]] && head -12 "$SOAK_DIR/LATEST.md"
    tail -n 5 "$log" 2>/dev/null
    ;;
  report)
    python3 -I "$here/soak_analyze.py" report "$state" "$SOAK_DIR/LATEST.md" && echo "$SOAK_DIR/LATEST.md"
    ;;
  *) sed -n 2,26p "$0"; exit 2 ;;
esac
