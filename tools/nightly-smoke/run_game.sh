#!/usr/bin/env bash
# Start one random UI smoke playthrough in the background.
#
#   run_game.sh start <run-dir>     pick random seeds, launch, print the run's parameters
#   run_game.sh stop  <run-dir>     kill the run (all of its processes)
#
# The run plays every seat through the browser until STOP_ROUND, game over, the first failure
# or DEADLINE (epoch seconds, from the proctor loop). Artifacts land in <run-dir>:
#   meta.json  run.log (Playwright + backend stderr: engine panics show up here)
#   trace/     game.json trace.jsonl report.json final-snapshot.json browser-errors.json failure.txt
#   exit_code  written when the run ends
set -euo pipefail
source "$(dirname "$0")/config.sh"

cmd="${1:-}"
run_dir="${2:-}"
[ -n "$cmd" ] && [ -n "$run_dir" ] || { echo "usage: $0 start|stop <run-dir>" >&2; exit 2; }

case "$cmd" in
start)
  if [ -f "$run_dir/run.pid" ]; then
    echo "run already started in $run_dir (pid $(cat "$run_dir/run.pid"))"
    cat "$run_dir/meta.json"
    exit 0
  fi
  mkdir -p "$run_dir/trace"
  game_seed=$(shuf -i 1-2000000000 -n 1)
  click_seed=$(shuf -i 1-2000000000 -n 1)
  players=$(shuf -e $PLAYER_COUNTS -n 1)
  policy=$(shuf -e $POLICIES -n 1)
  preset=""
  if [ "$(shuf -i 1-100 -n 1)" -le "$PRESET_PROBABILITY" ]; then
    preset=$(shuf -e $PRESET_NAME -n 1)
    if [ "$preset" != leaders ] && [ "$(shuf -i 1-100 -n 1)" -le "$PRESET_ROTATE_PERCENT" ]; then
      preset="$preset+rot"
    fi
  fi
  preset_env=""
  [ -z "$preset" ] || preset_env="TI4_SMOKE_PRESET=$preset "
  card_set=te
  [ "$(shuf -i 1-100 -n 1)" -le "$POK_PROBABILITY" ] && card_set=pok
  card_env=""
  [ "$card_set" = te ] || card_env="TI4_SMOKE_CARD_SET=$card_set "
  port=$(shuf -i 20000-49000 -n 1)
  deadline="${DEADLINE:-$(( $(now_epoch) + 6 * 3600 ))}"
  # The code under test: nothing keeps interactive commits off the night branch while a game plays,
  # so each run records the commit and how many files differ from it.
  commit=$(git -C "$REPO" rev-parse HEAD 2>/dev/null || echo unknown)
  dirty_files=$(git -C "$REPO" status --porcelain 2>/dev/null | wc -l)
  budget=$(( deadline - $(now_epoch) ))
  [ "$budget" -gt 60 ] || { echo "no time left before the deadline" >&2; exit 1; }
  cat > "$run_dir/meta.json" <<EOF
{
  "game_seed": $game_seed,
  "click_seed": $click_seed,
  "players": $players,
  "policy": "$policy",
  "preset": "$preset",
  "card_set": "$card_set",
  "commit": "$commit",
  "dirty_files": $dirty_files,
  "stop_round": $STOP_ROUND,
  "backend_port": $port,
  "server_data_dir": "/tmp/ti4-playwright-games-$port",
  "started_at": "$(TZ="$NIGHTLY_TZ" date '+%F %T %Z')",
  "deadline": "$(TZ="$NIGHTLY_TZ" date -d "@$deadline" '+%F %T %Z')",
  "repro": "cd web && TI4_SMOKE=1 ${preset_env}${card_env}TI4_SMOKE_PLAYERS=$players TI4_SMOKE_GAME_SEED=$game_seed TI4_SMOKE_CLICK_SEED=$click_seed TI4_SMOKE_POLICY=$policy TI4_SMOKE_ROUND=$STOP_ROUND TI4_SMOKE_DECISIONS=$MAX_DECISIONS npm run test:e2e:smoke"
}
EOF
  # A new session makes the run its own process group, so `stop` can kill browsers and the
  # Playwright web servers (cargo/vite) together.
  setsid "$NIGHTLY_DIR/_run_inner.sh" "$run_dir" "$players" "$game_seed" "$click_seed" \
    "$policy" "$port" "$budget" "$preset" "$card_set" </dev/null >/dev/null 2>&1 &
  echo $! > "$run_dir/run.pid"
  echo "started run in $run_dir"
  cat "$run_dir/meta.json"
  ;;
stop)
  if [ -f "$run_dir/run.pid" ] && [ ! -f "$run_dir/exit_code" ]; then
    pid=$(cat "$run_dir/run.pid")
    # Playwright starts its web servers and browsers in new process groups/sessions, so a group
    # kill misses them. Collect everything in the run's session plus all descendants first
    # (children are re-parented to init once their parent dies), then kill the whole set.
    descendants() { local child; for child in $(ps -o pid= --ppid "$1"); do echo "$child"; descendants "$child"; done; }
    procs=$( { echo "$pid"
      for p in $(ps -eo pid=,sid= | awk -v s="$pid" '$2 == s { print $1 }'); do echo "$p"; descendants "$p"; done
    } | sort -u)
    kill -TERM $procs 2>/dev/null || true
    port=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['backend_port'])" "$run_dir/meta.json")
    kill_run_processes "$port"
    kill -KILL $procs 2>/dev/null || true
    [ -f "$run_dir/exit_code" ] || echo "killed" > "$run_dir/exit_code"
    echo "stopped run in $run_dir"
  else
    echo "run in $run_dir is not running"
  fi
  ;;
*)
  echo "usage: $0 start|stop <run-dir>" >&2
  exit 2
  ;;
esac
