#!/usr/bin/env bash
# Started by run_game.sh under setsid; runs one smoke playthrough and records its exit code.
source "$(dirname "$0")/config.sh"
run_dir="$1" players="$2" game_seed="$3" click_seed="$4" policy="$5" port="$6" budget="$7" preset="${8:-}" card_set="${9:-te}"
cd "$REPO/web" || exit 1
export TI4_SMOKE=1 TI4_SMOKE_PLAYERS="$players" TI4_SMOKE_GAME_SEED="$game_seed" \
  TI4_SMOKE_CLICK_SEED="$click_seed" TI4_SMOKE_POLICY="$policy" \
  TI4_SMOKE_ROUND="$STOP_ROUND" TI4_SMOKE_DECISIONS="$MAX_DECISIONS" \
  TI4_SMOKE_TRACE_DIR="$run_dir/trace" TI4_SMOKE_PRESET="$preset" TI4_SMOKE_CARD_SET="$card_set" \
  TI4_E2E_BACKEND_PORT="$port" TI4_E2E_FRONTEND_PORT="$((port + 1))"
timeout --kill-after=30 "$budget" npx playwright test e2e/smoke_playthrough.spec.ts \
  --global-timeout=0 --reporter=line --output="$run_dir/playwright" > "$run_dir/run.log" 2>&1
code=$?
# Nothing of this run may outlive it (a deadline kill can orphan browsers and web servers).
kill_run_processes "$port"
echo "$code" > "$run_dir/exit_code"
# Keep the server's own record of the game next to the trace.
cp -r "/tmp/ti4-playwright-games-$port" "$run_dir/server-data" 2>/dev/null
exit 0
