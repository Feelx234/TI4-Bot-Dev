#!/usr/bin/env bash
# Nightly random-UI smoke sweep, proctored by Haiku and summarised by Opus in the morning.
#
#   nightly.sh tick      called by cron every 5 minutes; starts the sweep inside the window and
#                        writes the morning summary once the window has ended
#   nightly.sh loop      the sweep itself: one proctored run after another until END
#   nightly.sh summary   the Opus morning summary for the most recent window (read-only)
#
# Output: $REPORT_ROOT/<night>/report.md (one entry per run, appended by the proctors),
#         $REPORT_ROOT/<night>/summary.md (morning summary), runs/<NN-HHMM>/ (raw evidence).
set -uo pipefail
source "$(dirname "$0")/config.sh"

# A nested `claude -p` started from inside a Claude Code session must not inherit its session.
for var in $(env | grep -o '^CLAUDE[A-Z_]*' || true); do unset "$var"; done
unset AI_AGENT

read -r NIGHT START_EPOCH END_EPOCH < <(current_window)
NIGHT_DIR="$REPORT_ROOT/$NIGHT"
LOCK="$NIGHT_DIR/.loop.lock"
mkdir -p "$NIGHT_DIR"

render() { # render <template> KEY=VALUE...
  local text
  text=$(cat "$1")
  shift
  for pair in "$@"; do text=${text//"{{${pair%%=*}}}"/${pair#*=}}; done
  printf '%s' "$text"
}

sweep_running() { ! flock -n "$LOCK" true 2>/dev/null; }

cmd_loop() {
  exec 9>"$LOCK"
  flock -n 9 || { log "sweep already running for $NIGHT"; exit 0; }

  # Log when the loop exits via signal or explicit exit.
  trap 'log "sweep exited on signal or exit with $(now_epoch) (wall time: $(TZ="$NIGHTLY_TZ" date +"%F %T %Z"))"' EXIT

  local report="$NIGHT_DIR/report.md"
  [ -f "$report" ] || {
    echo "# Nightly UI smoke sweep — night of $NIGHT"
    echo
    echo "Window $START_HHMM–$END_HHMM $NIGHTLY_TZ, runs until round $STOP_ROUND, proctor $PROCTOR_MODEL."
    echo
  } > "$report"
  log "sweep for $NIGHT until $(TZ="$NIGHTLY_TZ" date -d "@$END_EPOCH" '+%F %T %Z')"

  # Build once so the first run's backend start does not eat into Playwright's server timeout.
  if ! (cd "$REPO" && cargo build --quiet -p ti4-server --bin server) > "$NIGHT_DIR/build.log" 2>&1; then
    { echo "## Build failed — no runs tonight"; echo '```'; tail -n 60 "$NIGHT_DIR/build.log"; echo '```'; } >> "$report"
    log "build failed"
    touch "$NIGHT_DIR/sweep.done"
    exit 1
  fi

  local n=0
  while [ $(( END_EPOCH - $(now_epoch) )) -gt "$MIN_RUN_SECONDS" ]; do
    n=$((n + 1))
    local run_name
    run_name="$(printf '%02d' "$n")-$(TZ="$NIGHTLY_TZ" date +%H%M)"
    local run_dir="$NIGHT_DIR/runs/$run_name"
    mkdir -p "$run_dir"
    log "run $run_name: launching proctor"
    local prompt
    prompt=$(render "$NIGHTLY_DIR/prompts/proctor.md" "RUN_DIR=$run_dir" "TOOLS=$NIGHTLY_DIR" "RUN_NAME=$run_name")
    # The game itself stops 5 minutes before END so the proctor can still write its entry.
    export DEADLINE=$(( END_EPOCH - 300 ))
    timeout --kill-after=30 $(( END_EPOCH - $(now_epoch) + 600 )) \
      "$CLAUDE_BIN" -p --model "$PROCTOR_MODEL" --no-session-persistence \
      --permission-mode dontAsk --tools Bash Read Grep Glob \
      --allowedTools "Bash($NIGHTLY_DIR/run_game.sh:*)" "Bash($NIGHTLY_DIR/watch.sh:*)" \
      "Bash(python3 $NIGHTLY_DIR/digest.py:*)" "Read" "Grep" "Glob" \
      --disallowedTools "Edit" "Write" "NotebookEdit" "Agent" \
      -- "$prompt" > "$run_dir/proctor-entry.md" 2> "$run_dir/proctor.err" < /dev/null
    local status=$?
    # Whatever the proctor did, never leave a game running.
    "$NIGHTLY_DIR/run_game.sh" stop "$run_dir" > /dev/null 2>&1
    python3 "$NIGHTLY_DIR/digest.py" "$run_dir" > "$run_dir/digest.md" 2>&1
    if [ -s "$run_dir/proctor-entry.md" ] && [ -f "$run_dir/meta.json" ]; then
      { cat "$run_dir/proctor-entry.md"; echo; echo "_Raw evidence: \`$run_dir\`_"; echo; } >> "$report"
    else
      {
        echo "## Run $run_name — proctor failed (exit $status), mechanical digest only"
        echo
        sed -n '1,20p' "$run_dir/proctor.err" | sed 's/^/    /'
        echo
        cat "$run_dir/digest.md"
        echo
      } >> "$report"
    fi
    log "run $run_name: done (proctor exit $status)"
    # A run that never started (e.g. the proctor failed immediately) must not spin the loop.
    [ -f "$run_dir/meta.json" ] || sleep 60
  done
  touch "$NIGHT_DIR/sweep.done"
  log "sweep finished after $n runs"
}

cmd_summary() {
  if [ -s "$NIGHT_DIR/summary.md" ] && [ -z "${FORCE:-}" ]; then
    log "summary for $NIGHT already exists"
    exit 0
  fi
  # Let the last proctor finish its entry.
  exec 9>"$LOCK"
  flock -w 1800 9 || log "sweep still running after 30 minutes; summarising what is there"
  [ -f "$NIGHT_DIR/report.md" ] || { log "no report for $NIGHT"; exit 0; }
  log "writing morning summary for $NIGHT"
  local prompt
  prompt=$(render "$NIGHTLY_DIR/prompts/summary.md" "REPORT=$NIGHT_DIR/report.md" \
    "NIGHT_DIR=$NIGHT_DIR/runs" "REPO=$REPO" "STOP_ROUND=$STOP_ROUND")
  (cd "$REPO" && timeout 3600 "$CLAUDE_BIN" -p --model "$SUMMARY_MODEL" --no-session-persistence \
    --permission-mode dontAsk --tools Bash Read Grep Glob \
    --allowedTools "Read" "Grep" "Glob" "Bash(git log:*)" "Bash(git show:*)" "Bash(git diff:*)" \
    --disallowedTools "Edit" "Write" "NotebookEdit" "Agent" \
    -- "$prompt" > "$NIGHT_DIR/summary.md.tmp" 2> "$NIGHT_DIR/summary.err" < /dev/null)
  local status=$?
  if [ "$status" -eq 0 ] && [ -s "$NIGHT_DIR/summary.md.tmp" ]; then
    mv "$NIGHT_DIR/summary.md.tmp" "$NIGHT_DIR/summary.md"
    ln -sfn "$NIGHT_DIR/summary.md" "$REPORT_ROOT/latest-summary.md"
    log "summary written: $NIGHT_DIR/summary.md"
  else
    log "summary failed (exit $status), see $NIGHT_DIR/summary.err"
  fi
}

cmd_tick() {
  local now
  now=$(now_epoch)
  if [ "$now" -ge "$START_EPOCH" ] && [ $(( END_EPOCH - now )) -gt "$MIN_RUN_SECONDS" ] \
    && [ ! -f "$NIGHT_DIR/sweep.done" ] && ! sweep_running; then
    log "tick: starting sweep for $NIGHT"
    setsid "$0" loop >> "$NIGHT_DIR/sweep.log" 2>&1 < /dev/null &
  elif [ "$now" -ge "$END_EPOCH" ] && [ $(( now - END_EPOCH )) -lt 14400 ] \
    && [ ! -s "$NIGHT_DIR/summary.md" ] && [ ! -f "$NIGHT_DIR/.summary-started" ]; then
    touch "$NIGHT_DIR/.summary-started"
    log "tick: starting morning summary for $NIGHT"
    setsid "$0" summary >> "$NIGHT_DIR/sweep.log" 2>&1 < /dev/null &
  fi
}

case "${1:-}" in
  tick) cmd_tick ;;
  loop) cmd_loop ;;
  summary) cmd_summary ;;
  *) echo "usage: $0 tick|loop|summary" >&2; exit 2 ;;
esac
