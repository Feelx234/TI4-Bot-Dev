#!/usr/bin/env bash
# Nightly random-UI smoke sweep: Sonnet proctors play games from 20:30 to 06:00 Berlin, Opus fixes
# the first real findings at 23:59 (round 1, earlier when a proctor asks via request_fix.sh) and again after the sweep (round 2), and an Opus
# morning summary follows round 2. See README.md.
#
#   nightly.sh tick              called by cron every 5 minutes; starts whatever is due
#   nightly.sh tick-decide       prints what tick would start, without starting it (tests)
#   nightly.sh loop [night]      the sweep itself: one proctored run after another until END
#   nightly.sh fix <round> [night]   an Opus fix round, working in its own git worktree
#   nightly.sh summary [night]   the Opus morning summary (read-only)
#
# Output: $REPORT_ROOT/<night>/report.md (one entry per run, appended by the proctors),
#         fixer-<round>.md (fix reports), summary.md (morning summary), runs/<NN-HHMM>/ (evidence).
set -uo pipefail
source "$(dirname "$0")/config.sh"

# A nested `claude -p` started from inside a Claude Code session must not inherit its session.
for var in $(env | grep -o '^CLAUDE[A-Z_]*' || true); do unset "$var"; done
unset AI_AGENT

set_night() {
  NIGHT="$1"
  read -r START_EPOCH END_EPOCH < <(window_for_night "$NIGHT")
  NIGHT_DIR="$REPORT_ROOT/$NIGHT"
  LOCK="$NIGHT_DIR/.loop.lock"
  FIX_BRANCH="nightly-fixes-$NIGHT"
}
default_night() { current_window | cut -d' ' -f1; }

render() { # render <template> KEY=VALUE...
  local text
  text=$(cat "$1")
  shift
  for pair in "$@"; do text=${text//"{{${pair%%=*}}}"/"${pair#*=}"}; done
  printf '%s' "$text"
}

lock_held() { [ -e "$1" ] && ! flock -n "$1" true 2>/dev/null; }
sweep_running() { lock_held "$LOCK"; }
fixer_enabled() { case " $FIXERS " in *" $1 "*) return 0 ;; esac; return 1; }
report_entries() {
  local n=0
  [ -f "$NIGHT_DIR/report.md" ] && n=$(grep -c '^## Run ' "$NIGHT_DIR/report.md" || true)
  echo "$n"
}
fix1_epoch() {
  local e
  e=$(day_epoch "$NIGHT" "$FIX1_HHMM")
  [ "$e" -lt "$START_EPOCH" ] && e=$(day_epoch "$(shift_day "$NIGHT" +1)" "$FIX1_HHMM")
  echo "$e"
}
# The morning summary waits for fix round 2 (done, skipped, disabled, or hopelessly late).
fixer2_gate_open() {
  fixer_enabled 2 || return 0
  [ -f "$NIGHT_DIR/.fixer-2.done" ] && return 0
  [ "$(now_epoch)" -ge $(( END_EPOCH + FIX2_MAX_SECONDS + 1800 )) ] && return 0
  return 1
}
# Round 2 and the summary of the previous night use the same checkout as the next sweep, so the
# sweep waits while either is running or is already due (decide_night prints an action for it).
previous_pipeline_busy() (
  set_night "$(shift_day "$NIGHT" -1)"
  lock_held "$NIGHT_DIR/.fixer-1.lock" || lock_held "$NIGHT_DIR/.fixer-2.lock" \
    || lock_held "$NIGHT_DIR/.summary.lock" || [ -n "$(decide_night "$NIGHT" 0)" ]
)

# ---------------------------------------------------------------------------------------------
# Hand-off of fixer branches into the night branch. The fixers work in their own worktrees; their
# commits arrive in $REPO only here, between games, so no game ever runs on changing sources.
# ---------------------------------------------------------------------------------------------
note_report() { echo "$*" >> "$NIGHT_DIR/report.md"; echo >> "$NIGHT_DIR/report.md"; }
note_once() { # note_once <round> <text>
  [ -f "$NIGHT_DIR/.merge-note-$1" ] && return 0
  touch "$NIGHT_DIR/.merge-note-$1"
  note_report "## Fixer round $1 not merged yet: $2"
  log "fixer round $1: $2"
}
MERGED_NOW=""
merge_pending() { # merge_pending <verify: 1 builds after each merge and drops a merge that breaks it>
  local verify="$1" r branch pre count
  for r in 1 2; do
    [ -f "$NIGHT_DIR/fixer-$r.ready" ] || continue
    branch=$(cat "$NIGHT_DIR/fixer-$r.ready")
    if [ "$(git -C "$REPO" branch --show-current)" != "$FIX_BRANCH" ]; then
      note_once "$r" "the checkout is on '$(git -C "$REPO" branch --show-current)', not on $FIX_BRANCH; branch $branch is kept for the user"
      continue
    fi
    pre=$(git -C "$REPO" rev-parse HEAD)
    count=$(git -C "$REPO" rev-list --count "$pre..$branch")
    if git -C "$REPO" merge --no-edit -q "$branch" >> "$NIGHT_DIR/merge.log" 2>&1; then
      mv "$NIGHT_DIR/fixer-$r.ready" "$NIGHT_DIR/.fixer-$r.merged"
      MERGED_NOW="$MERGED_NOW $r"
      note_report "## Fixer round $r: merged $branch ($count commits) into $FIX_BRANCH"
      log "fixer round $r: merged $branch ($count commits)"
      if [ "$verify" = 1 ] && ! (cd "$REPO" && bash -c "$BUILD_CMD") >> "$NIGHT_DIR/merge.log" 2>&1; then
        git -C "$REPO" reset -q --hard "$pre" >> "$NIGHT_DIR/merge.log" 2>&1
        mv "$NIGHT_DIR/.fixer-$r.merged" "$NIGHT_DIR/.fixer-$r.rejected"
        note_report "## Fixer round $r: the merge broke the build and was dropped (branch $branch is kept)"
        log "fixer round $r: merge broke the build, dropped"
      fi
    else
      git -C "$REPO" merge --abort >> "$NIGHT_DIR/merge.log" 2>&1 || true
      mv "$NIGHT_DIR/fixer-$r.ready" "$NIGHT_DIR/.fixer-$r.conflict"
      note_report "## Fixer round $r: merge of $branch conflicted and was aborted (branch is kept for the user)"
      log "fixer round $r: merge conflict, aborted"
    fi
  done
}

# ---------------------------------------------------------------------------------------------
# The sweep
# ---------------------------------------------------------------------------------------------
# Highest NN of the existing runs/NN-HHMM directories, 0 when there are none. A restarted sweep
# (cron after a kill) continues from it, so run names stay unique within a night. Gaps and
# directories that do not match the pattern are ignored.
highest_run_number() { # highest_run_number <runs-dir>
  local d name max=0 num
  for d in "$1"/*/; do
    [ -d "$d" ] || continue
    name=$(basename "$d")
    [[ "$name" =~ ^([0-9]+)-[0-9]{4}$ ]] || continue
    num=$((10#${BASH_REMATCH[1]}))
    [ "$num" -gt "$max" ] && max=$num
  done
  echo "$max"
}

# sleep that a signal can interrupt at once (a foreground sleep would delay the traps).
nap() { sleep "$1" & wait $! 2>/dev/null; }

# State of the run in progress, read by the signal/exit trap.
ACTIVE_RUN_DIR="" ACTIVE_RUN_NAME="" PROCTOR_PID="" CLEANUP_DONE=0

# Kill the proctor's whole session (subshell, timeout, claude and everything claude started).
kill_proctor_session() {
  local pid="$1" sig pids
  [ -n "$pid" ] || return 0
  for sig in TERM KILL; do
    pids=$(ps -eo pid=,sid= | awk -v s="$pid" '$2 == s { print $1 }')
    [ -n "$pids" ] || return 0
    kill -"$sig" $pids 2>/dev/null || true
    [ "$sig" = TERM ] && nap 3
  done
  return 0
}

# Runs on SIGTERM/SIGINT/SIGHUP and on every exit of the loop. Idempotent (CLEANUP_DONE); does
# nothing when no run is active. With a run active: stop the proctor and the game, build the
# mechanical digest and append an entry marked "sweep terminated mid-run".
sweep_cleanup() { # sweep_cleanup <reason>
  [ "$CLEANUP_DONE" = 1 ] && return 0
  CLEANUP_DONE=1
  trap '' TERM INT HUP # a second signal must not interrupt the cleanup
  kill_proctor_session "$PROCTOR_PID"
  PROCTOR_PID=""
  local run_dir="$ACTIVE_RUN_DIR" name="$ACTIVE_RUN_NAME"
  ACTIVE_RUN_DIR=""
  [ -n "$run_dir" ] || return 0
  timeout --kill-after=5 60 "$NIGHTLY_DIR/run_game.sh" stop "$run_dir" > /dev/null 2>&1
  timeout --kill-after=5 120 python3 "$NIGHTLY_DIR/digest.py" "$run_dir" > "$run_dir/digest.md" 2>&1
  local last="unknown"
  if [ -s "$run_dir/trace/trace.jsonl" ]; then
    last=$(tail -n 1 "$run_dir/trace/trace.jsonl" | python3 -c \
      'import json,sys; r=json.load(sys.stdin); print("#%s, round %s" % (r.get("decision","?"), r.get("round","?")))' 2>/dev/null) || last="unknown"
    [ -n "$last" ] || last="unknown"
  fi
  {
    echo "## Run $name — sweep terminated mid-run ($1), proctor failed, mechanical digest only"
    echo
    echo "The sweep process ended while this run was active; the game and the proctor were stopped and there is no proctor entry. Last decision: $last. Evidence: \`$run_dir\`."
    if [ -s "$run_dir/proctor-entry.md" ]; then echo "A partial proctor entry was kept in \`$run_dir/proctor-entry.md\`."; fi
    echo
    cat "$run_dir/digest.md"
    echo
  } >> "$NIGHT_DIR/report.md"
  log "run $name: sweep terminated mid-run ($1); game stopped, digest appended"
}
log_loop_exit() { log "sweep exited on signal or exit (wall time: $(TZ="$NIGHTLY_TZ" date -d "@$(now_epoch)" +"%F %T %Z"))"; }

cmd_loop() {
  set_night "${1:-$(default_night)}"
  mkdir -p "$NIGHT_DIR"
  exec 9>"$LOCK"
  flock -n 9 || { log "sweep already running for $NIGHT"; exit 0; }

  # Log when the loop exits via signal or explicit exit.
  # A signal first stops the active run and files its entry (sweep_cleanup), then exits, which runs
  # the EXIT trap; the cleanup runs only once. sweep.done is not written: cron restarts the sweep.
  trap 'sweep_cleanup exit; log_loop_exit' EXIT
  trap 'sweep_cleanup SIGTERM; exit 143' TERM
  trap 'sweep_cleanup SIGINT; exit 130' INT
  trap 'sweep_cleanup SIGHUP; exit 129' HUP

  local report="$NIGHT_DIR/report.md"
  [ -f "$report" ] || {
    echo "# Nightly UI smoke sweep — night of $NIGHT"
    echo
    echo "Window $START_HHMM–$END_HHMM $NIGHTLY_TZ, runs until round $STOP_ROUND, proctor $PROCTOR_MODEL, fixer rounds: ${FIXERS:-none} ($FIXER_MODEL)."
    echo
  } > "$report"
  log "sweep for $NIGHT until $(TZ="$NIGHTLY_TZ" date -d "@$END_EPOCH" '+%F %T %Z')"

  # The whole night runs on one branch, nightly-fixes-<night>, checked out in $REPO. Before
  # switching, the current state (uncommitted and untracked work included) is committed onto it,
  # so nothing is lost. Proctors commit their minor repairs here, and the Opus fixers' branches are
  # merged into it between games, so every later run includes the earlier fixes. Nothing is
  # pushed. In the morning: git checkout <orig_branch> and merge/cherry-pick.
  if [ "$(git -C "$REPO" branch --show-current)" != "$FIX_BRANCH" ]; then
    git -C "$REPO" branch --show-current > "$NIGHT_DIR/orig_branch"
    if git -C "$REPO" show-ref -q --verify "refs/heads/$FIX_BRANCH"; then
      git -C "$REPO" checkout -q "$FIX_BRANCH"
    else
      git -C "$REPO" checkout -q -b "$FIX_BRANCH" \
        && git -C "$REPO" add -A \
        && { git -C "$REPO" diff --cached --quiet || git -C "$REPO" commit -q -m "nightly $NIGHT: snapshot of the working tree before the sweep"; }
    fi >> "$NIGHT_DIR/build.log" 2>&1 || {
      { echo "## Could not switch to $FIX_BRANCH — no runs tonight"; echo '```'; tail -n 30 "$NIGHT_DIR/build.log"; echo '```'; } >> "$report"
      log "branch setup failed"; touch "$NIGHT_DIR/sweep.done"; exit 1
    }
  fi
  BASE_COMMIT=$(git -C "$REPO" rev-parse HEAD)

  # Continue the numbering of an earlier sweep of this night (cron restarts after a kill).
  local n disk_waits=0
  n=$(highest_run_number "$NIGHT_DIR/runs")
  while [ $(( END_EPOCH - $(now_epoch) )) -gt "$MIN_RUN_SECONDS" ]; do
    # No run (and no run directory) without free disk: a full disk used to turn the rest of a night
    # into phantom runs and a truncated report. Wait for space, and give up after MAX_DISK_WAITS.
    if ! disk_ok; then
      disk_waits=$((disk_waits + 1))
      if [ "$disk_waits" -eq 1 ]; then
        note_report "## Waiting for disk space: $(free_gb) GB free, runs need $MIN_FREE_GB GB (checked every ${DISK_WAIT_SECONDS}s)"
      fi
      log "low disk: $(free_gb) GB free, need $MIN_FREE_GB GB (check $disk_waits of $MAX_DISK_WAITS)"
      if [ "$disk_waits" -ge "$MAX_DISK_WAITS" ]; then
        note_report "## Sweep stopped: the disk stayed below $MIN_FREE_GB GB for $MAX_DISK_WAITS checks"
        log "sweep stopped: no disk space"; break
      fi
      nap "$DISK_WAIT_SECONDS"; continue
    fi
    disk_waits=0
    n=$((n + 1))
    local run_name
    run_name="$(printf '%02d' "$n")-$(TZ="$NIGHTLY_TZ" date -d "@$(now_epoch)" +%H%M)"
    local run_dir="$NIGHT_DIR/runs/$run_name"
    if ! mkdir -p "$run_dir"; then
      note_report "## Run $run_name not started: cannot create $run_dir"
      log "run $run_name: cannot create the run directory"
      n=$((n - 1)); nap "$DISK_WAIT_SECONDS"; continue
    fi
    # Fixer branches that are ready come in now, between games, never during one.
    MERGED_NOW=""
    merge_pending 0
    # Build before every run so Playwright's backend start stays fast after a repair. A repair or
    # merge that broke the build is dropped (reset to the last good commit), so one bad fix
    # cannot cost the rest of the night.
    if ! (cd "$REPO" && bash -c "$BUILD_CMD") > "$run_dir/build.log" 2>&1; then
      if [ "$(git -C "$REPO" rev-parse HEAD)" != "$BASE_COMMIT" ]; then
        # Only proctor commits and fixer merges exist past the last good build; drop them (they
        # stay in the reflog and in the fixer branches).
        git -C "$REPO" reset -q --hard "$BASE_COMMIT" >> "$run_dir/build.log" 2>&1
        local r
        for r in $MERGED_NOW; do mv "$NIGHT_DIR/.fixer-$r.merged" "$NIGHT_DIR/.fixer-$r.rejected" 2>/dev/null; done
        { echo "## Before run $run_name: a proctor repair or fixer merge broke the build; branch reset to the last good commit${MERGED_NOW:+ (fixer round(s)$MERGED_NOW rejected)}"; echo; } >> "$report"
        log "run $run_name: build broke, reset to last good commit"
        rm -rf "$run_dir"; n=$((n - 1)); continue
      fi
      { echo "## Build failed — no runs tonight"; echo '```'; tail -n 60 "$run_dir/build.log"; echo '```'; } >> "$report"
      log "build failed"; break
    fi
    BASE_COMMIT=$(git -C "$REPO" rev-parse HEAD)
    log "run $run_name: launching proctor"
    local prompt
    prompt=$(render "$NIGHTLY_DIR/prompts/proctor.md" "RUN_DIR=$run_dir" "TOOLS=$NIGHTLY_DIR" "RUN_NAME=$run_name" "FIX_BRANCH=$FIX_BRANCH")
    # The game itself stops 5 minutes before END so the proctor can still write its entry.
    export DEADLINE=$(( END_EPOCH - 300 ))
    # Auto mode with edit tools, working in $REPO on the night's branch. Pushing, switching
    # branches and rewriting history stay denied.
    ACTIVE_RUN_DIR="$run_dir" ACTIVE_RUN_NAME="$run_name"
    # Background job in its own session (setsid) + wait: a signal reaches the traps at once and
    # sweep_cleanup can kill claude and everything it started.
    (cd "$REPO" && exec setsid timeout --kill-after=30 $(( END_EPOCH - $(now_epoch) + 600 )) \
      "$CLAUDE_BIN" -p --model "$PROCTOR_MODEL" --no-session-persistence \
      --permission-mode auto --tools Bash Read Grep Glob Edit Write \
      --allowedTools "Bash($NIGHTLY_DIR/run_game.sh:*)" "Bash($NIGHTLY_DIR/watch.sh:*)" "Bash($NIGHTLY_DIR/request_fix.sh:*)" \
      "Bash(python3 $NIGHTLY_DIR/digest.py:*)" "Read" "Grep" "Glob" "Edit" "Write" \
      "Bash(git status:*)" "Bash(git diff:*)" "Bash(git add:*)" "Bash(git commit:*)" "Bash(git log:*)" \
      "Bash(cargo check:*)" "Bash(cargo test:*)" "Bash(cargo fmt:*)" "Bash(npm test:*)" "Bash(npx tsc:*)" "Bash(npx vitest:*)" \
      --disallowedTools "NotebookEdit" "Agent" "Bash(git push:*)" "Bash(git checkout:*)" "Bash(git switch:*)" \
      "Bash(git reset:*)" "Bash(git rebase:*)" "Bash(git merge:*)" "Bash(git branch:*)" "Bash(git worktree:*)" \
      "Bash(git stash:*)" "Bash(git clean:*)" "Bash(git revert:*)" "Bash(rm:*)" "Bash(sudo:*)" "Bash(kill:*)" "Bash(pkill:*)" \
      -- "$prompt" > "$run_dir/proctor-entry.md" 2> "$run_dir/proctor.err" < /dev/null) &
    PROCTOR_PID=$!
    wait "$PROCTOR_PID"
    local status=$?
    PROCTOR_PID=""
    # Whatever the proctor did, never leave a game running.
    "$NIGHTLY_DIR/run_game.sh" stop "$run_dir" > /dev/null 2>&1
    python3 "$NIGHTLY_DIR/digest.py" "$run_dir" > "$run_dir/digest.md" 2>&1
    ACTIVE_RUN_DIR="" # from here on the entry is written by this loop, not by the trap
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
    [ -f "$run_dir/meta.json" ] || nap "${NIGHTLY_FAILED_RUN_SLEEP:-60}"
  done
  touch "$NIGHT_DIR/sweep.done"
  log "sweep finished after $n runs"
}

# ---------------------------------------------------------------------------------------------
# An Opus fix round
# ---------------------------------------------------------------------------------------------
cmd_fix() {
  local round="${1:?usage: nightly.sh fix <round> [night]}"
  set_night "${2:-$(default_night)}"
  mkdir -p "$NIGHT_DIR"
  exec 8>"$NIGHT_DIR/.fixer-$round.lock"
  flock -n 8 || { log "fixer round $round already running for $NIGHT"; exit 0; }
  touch "$NIGHT_DIR/.fixer-$round-started"
  # The exit trap tells a finished round from one that never got going (no disk, no worktree): the
  # second is reported and retried, see fixer_exit.
  FIX_ROUND="$round" FIX_OK=0 FIX_FAIL_REASON=""
  trap fixer_exit EXIT

  local max="$FIX1_MAX_SECONDS"
  [ "$round" -ge 2 ] && max="$FIX2_MAX_SECONDS"
  local fixer_branch="nightly-opus-$NIGHT-r$round" wt="$NIGHT_DIR/fixer-$round" base status

  # With no sweep running, earlier rounds' ready branches come in first (and are build-checked),
  # so this round builds on them.
  if ! sweep_running; then merge_pending 1; fi

  if git -C "$REPO" show-ref -q --verify "refs/heads/$FIX_BRANCH"; then
    base="$FIX_BRANCH"
  else
    base=$(git -C "$REPO" rev-parse HEAD)
  fi
  if ! disk_ok; then
    FIX_FAIL_REASON="only $(free_gb) GB free, need $MIN_FREE_GB GB"
    log "fixer round $round: $FIX_FAIL_REASON"; exit 1
  fi
  if [ ! -d "$wt" ]; then
    git -C "$REPO" worktree add -q -B "$fixer_branch" "$wt" "$base" >> "$NIGHT_DIR/fixer-$round.err" 2>&1 \
      || { FIX_FAIL_REASON="cannot create worktree $wt ($(tail -n 1 "$NIGHT_DIR/fixer-$round.err" 2>/dev/null))"
           log "fixer round $round: $FIX_FAIL_REASON"; exit 1; }
  fi
  git -C "$wt" rev-parse HEAD > "$NIGHT_DIR/fixer-$round.base"
  if [ -d "$REPO/web/node_modules" ] && [ ! -e "$wt/web/node_modules" ]; then
    ln -sfn "$REPO/web/node_modules" "$wt/web/node_modules"
    local exclude
    exclude=$(git -C "$wt" rev-parse --git-path info/exclude)
    grep -qxF '/web/node_modules' "$exclude" 2>/dev/null || echo '/web/node_modules' >> "$exclude"
  fi

  log "fixer round $round: working in $wt on $fixer_branch ($FIXER_MODEL, budget ${max}s)"
  # A round started because a proctor asked (request_fix.sh): say so in the prompt and report.
  # Round 1 reads fix-requested; round 2 reads fix-requested-2 (a request made once round 1 had
  # started), also when it only starts after the sweep.
  local request="" req_reason req_run req_file="$NIGHT_DIR/fix-requested" early=" early"
  [ "$round" -ge 2 ] && { req_file="$NIGHT_DIR/fix-requested-2"; early=""; }
  if [ -s "$req_file" ]; then
    req_reason=$(sed -n '/^reason: /,$p' "$req_file" | sed '1s/^reason: //')
    req_run=$(sed -n 's/^run: //p' "$req_file" | head -1)
    request="A proctor (run ${req_run:-unknown}) asked for this round${early} because: $req_reason"
    note_report "## Fixer round $round started${early} on a proctor's request (run ${req_run:-unknown}): $req_reason"
    log "fixer round $round: started${early} on request: $req_reason"
  fi
  local prompt
  prompt=$(render "$NIGHTLY_DIR/prompts/fixer.md" "ROUND=$round" "REPORT=$NIGHT_DIR/report.md" \
    "RUNS_DIR=$NIGHT_DIR/runs" "FIXER_BRANCH=$fixer_branch" "MAX_FIXES=$MAX_FIXES_PER_ROUND" \
    "BUDGET_MINUTES=$(( max / 60 ))" "REQUEST=$request")
  # Own worktree, own target dir, low priority: the games keep their memory and CPU.
  (cd "$wt" && CARGO_TARGET_DIR="$REPORT_ROOT/target-fixer" CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 \
    nice -n 10 timeout --kill-after=60 "$max" \
    "$CLAUDE_BIN" -p --model "$FIXER_MODEL" --no-session-persistence \
    --permission-mode auto --tools Bash Read Grep Glob Edit Write \
    --allowedTools "Read" "Grep" "Glob" "Edit" "Write" \
    "Bash(git status:*)" "Bash(git diff:*)" "Bash(git add:*)" "Bash(git commit:*)" "Bash(git log:*)" \
    "Bash(cargo check:*)" "Bash(cargo test:*)" "Bash(cargo fmt:*)" "Bash(npm test:*)" "Bash(npx tsc:*)" "Bash(npx vitest:*)" \
    --disallowedTools "NotebookEdit" "Agent" "Bash(git push:*)" "Bash(git checkout:*)" "Bash(git switch:*)" \
    "Bash(git reset:*)" "Bash(git rebase:*)" "Bash(git merge:*)" "Bash(git branch:*)" "Bash(git worktree:*)" \
    "Bash(git stash:*)" "Bash(git clean:*)" "Bash(git revert:*)" "Bash(rm:*)" "Bash(sudo:*)" "Bash(kill:*)" "Bash(pkill:*)" \
    -- "$prompt" > "$NIGHT_DIR/fixer-$round.out" 2>> "$NIGHT_DIR/fixer-$round.err" < /dev/null)
  status=$?

  local commits
  commits=$(git -C "$wt" rev-list --count "$(cat "$NIGHT_DIR/fixer-$round.base")..HEAD" 2>/dev/null || echo 0)
  {
    echo "# Fixer round $round — night $NIGHT"
    echo
    echo "Model $FIXER_MODEL, exit $status, $commits commit(s) on \`$fixer_branch\`."
    echo
    if [ -s "$NIGHT_DIR/fixer-$round.out" ]; then
      cat "$NIGHT_DIR/fixer-$round.out"
    else
      echo "The fixer wrote no report (exit $status). Commits:"
      git -C "$wt" log --oneline "$(cat "$NIGHT_DIR/fixer-$round.base")..HEAD"
    fi
  } > "$NIGHT_DIR/fixer-$round.md"
  note_report "## Fixer round $round finished: $commits commit(s) on $fixer_branch, exit $status (report: fixer-$round.md)"
  [ "$commits" -gt 0 ] && echo "$fixer_branch" > "$NIGHT_DIR/fixer-$round.ready"
  # Once the sweep has ended nobody else would merge it; while it runs, the loop does, between games.
  if [ -f "$NIGHT_DIR/sweep.done" ] && ! sweep_running; then merge_pending 1; fi
  FIX_OK=1
}

# Exit trap of a fix round. A round that finished (FIX_OK) is done. One that never started working
# says so in the report and is retried after FIX_RETRY_SECONDS (decide_night), at most
# FIX_MAX_ATTEMPTS times; only then it counts as done so the morning summary is not held up.
fixer_exit() {
  local failed="$NIGHT_DIR/.fixer-$FIX_ROUND.failed" count=1 prev=""
  if [ "$FIX_OK" = 1 ]; then
    touch "$NIGHT_DIR/.fixer-$FIX_ROUND.done"
    log "fixer round $FIX_ROUND finished"
    return 0
  fi
  [ -s "$failed" ] && read -r prev _ < "$failed" && count=$((prev + 1))
  echo "$count $(now_epoch)" > "$failed"
  if [ "$count" -ge "$FIX_MAX_ATTEMPTS" ]; then
    touch "$NIGHT_DIR/.fixer-$FIX_ROUND.done"
    note_report "## Fixer round $FIX_ROUND failed (attempt $count of $FIX_MAX_ATTEMPTS, giving up): ${FIX_FAIL_REASON:-it stopped before doing any work}"
  else
    rm -f "$NIGHT_DIR/.fixer-$FIX_ROUND-started"
    note_report "## Fixer round $FIX_ROUND failed (attempt $count of $FIX_MAX_ATTEMPTS, retrying in ${FIX_RETRY_SECONDS}s): ${FIX_FAIL_REASON:-it stopped before doing any work}"
  fi
  log "fixer round $FIX_ROUND failed (attempt $count): ${FIX_FAIL_REASON:-stopped early}"
}
# A failed round waits FIX_RETRY_SECONDS before it is offered again.
fixer_retry_wait_over() { # fixer_retry_wait_over <round>
  local failed="$NIGHT_DIR/.fixer-$1.failed" count at
  [ -s "$failed" ] || return 0
  read -r count at < "$failed"
  [ "$(now_epoch)" -ge $(( ${at:-0} + FIX_RETRY_SECONDS )) ]
}

# ---------------------------------------------------------------------------------------------
# The morning summary
# ---------------------------------------------------------------------------------------------
cmd_summary() {
  set_night "${1:-$(default_night)}"
  mkdir -p "$NIGHT_DIR"
  exec 7>"$NIGHT_DIR/.summary.lock"
  flock -n 7 || { log "summary already running for $NIGHT"; exit 0; }
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
    "NIGHT_DIR=$NIGHT_DIR/runs" "NIGHT_ROOT=$NIGHT_DIR" "REPO=$REPO" "STOP_ROUND=$STOP_ROUND" \
    "FIX_BRANCH=$FIX_BRANCH" "FIXER1_BRANCH=nightly-opus-$NIGHT-r1" "FIXER2_BRANCH=nightly-opus-$NIGHT-r2")
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

# ---------------------------------------------------------------------------------------------
# tick: decide what is due. Looks at the previous night too, whose round 2 and summary run after
# the next window has already started (the gap before the next 20:30 start is long).
# ---------------------------------------------------------------------------------------------
decide_night() { # decide_night <night> <current: 1|0>; prints "<action> <night> [round]"
  set_night "$1"
  local current="$2" now
  now=$(now_epoch)
  [[ "$NIGHT" < "$NOT_BEFORE" ]] && return 0

  if [ "$current" = 1 ] && [ "$now" -ge "$START_EPOCH" ] && [ $(( END_EPOCH - now )) -gt "$MIN_RUN_SECONDS" ] \
    && [ ! -f "$NIGHT_DIR/sweep.done" ] && ! sweep_running; then
    if previous_pipeline_busy && [ "$now" -lt $(( START_EPOCH + START_DEFER_SECONDS )) ]; then
      return 0
    fi
    echo "sweep $NIGHT"
    return 0
  fi

  if [ "$current" = 1 ] && fixer_enabled 1 && [ ! -f "$NIGHT_DIR/.fixer-1-started" ] \
    && { [ "$now" -ge "$(fix1_epoch)" ] || [ -s "$NIGHT_DIR/fix-requested" ]; } && [ "$now" -lt "$END_EPOCH" ] \
    && fixer_retry_wait_over 1 \
    && [ ! -f "$NIGHT_DIR/sweep.done" ] && [ "$(report_entries)" -ge 1 ]; then
    echo "fix $NIGHT 1"
    return 0
  fi

  # A second early request (made once round 1 had started): round 2 starts as soon as round 1 has
  # finished, while the sweep keeps running. Its branch is merged between games like round 1's.
  if [ "$current" = 1 ] && fixer_enabled 2 && [ ! -f "$NIGHT_DIR/.fixer-2-started" ] \
    && [ -s "$NIGHT_DIR/fix-requested-2" ] && { ! fixer_enabled 1 || [ -f "$NIGHT_DIR/.fixer-1.done" ]; } \
    && [ "$now" -lt "$END_EPOCH" ] && fixer_retry_wait_over 2 \
    && [ ! -f "$NIGHT_DIR/sweep.done" ] && [ "$(report_entries)" -ge 1 ]; then
    echo "fix $NIGHT 2"
    return 0
  fi

  if fixer_enabled 2 && [ ! -f "$NIGHT_DIR/.fixer-2-started" ] && [ -f "$NIGHT_DIR/sweep.done" ] \
    && fixer_retry_wait_over 2 \
    && ! sweep_running && ! lock_held "$NIGHT_DIR/.fixer-1.lock"; then
    if [ "$(report_entries)" -ge 1 ]; then echo "fix $NIGHT 2"; else echo "skip-fix $NIGHT 2"; fi
    return 0
  fi

  if [ "$now" -ge "$END_EPOCH" ] && [ $(( now - END_EPOCH )) -lt $(( FIX2_MAX_SECONDS + 14400 )) ] \
    && [ ! -s "$NIGHT_DIR/summary.md" ] && [ ! -f "$NIGHT_DIR/.summary-started" ] && fixer2_gate_open; then
    echo "summary $NIGHT"
  fi
}
decide_all() {
  local cur
  cur=$(default_night)
  decide_night "$(shift_day "$cur" -1)" 0
  decide_night "$cur" 1
}

cmd_tick() {
  local action night arg
  while read -r action night arg; do
    [ -n "${action:-}" ] || continue
    set_night "$night"
    mkdir -p "$NIGHT_DIR"
    case "$action" in
      sweep)
        log "tick: starting sweep for $NIGHT"
        setsid "$0" loop "$NIGHT" >> "$NIGHT_DIR/sweep.log" 2>&1 < /dev/null &
        ;;
      fix)
        touch "$NIGHT_DIR/.fixer-$arg-started"
        log "tick: starting fixer round $arg for $NIGHT"
        setsid "$0" fix "$arg" "$NIGHT" >> "$NIGHT_DIR/sweep.log" 2>&1 < /dev/null &
        ;;
      skip-fix)
        touch "$NIGHT_DIR/.fixer-$arg-started" "$NIGHT_DIR/.fixer-$arg.done"
        log "tick: no run entries for $NIGHT, skipping fixer round $arg"
        ;;
      summary)
        touch "$NIGHT_DIR/.summary-started"
        log "tick: starting morning summary for $NIGHT"
        setsid "$0" summary "$NIGHT" >> "$NIGHT_DIR/sweep.log" 2>&1 < /dev/null &
        ;;
    esac
  done < <(decide_all)
}

case "${1:-}" in
  tick) cmd_tick ;;
  tick-decide) decide_all ;;
  loop) cmd_loop "${2:-}" ;;
  fix) cmd_fix "${2:-}" "${3:-}" ;;
  summary) cmd_summary "${2:-}" ;;
  *) echo "usage: $0 tick|tick-decide|loop [night]|fix <round> [night]|summary [night]" >&2; exit 2 ;;
esac
