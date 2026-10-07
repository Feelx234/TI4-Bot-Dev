# Nightly UI smoke sweep configuration. Every value can be overridden from the environment,
# which is how the short test run shrinks the window (see README.md).

REPO="${NIGHTLY_REPO:-/root/TI4-Bot-Dev}"
NIGHTLY_DIR="$REPO/tools/nightly-smoke"
REPORT_ROOT="${NIGHTLY_REPORT_ROOT:-$REPO/nightly-reports}"

# Window in Berlin local time. The sweep starts at START (one day) and runs until END (the next
# morning); it stops launching runs so that the last proctor can finish by END. A night is named
# after the Berlin date on which its window started.
NIGHTLY_TZ="${NIGHTLY_TZ:-Europe/Berlin}"
START_HHMM="${NIGHTLY_START:-20:30}"
END_HHMM="${NIGHTLY_END:-06:00}"
# Nights that started before this date (YYYY-MM-DD) are skipped by `tick`, so a new schedule never
# switches a checkout people are working in half way through a day.
NOT_BEFORE="${NIGHTLY_NOT_BEFORE:-2026-10-06}"

# Opus fix rounds. FIXERS lists the rounds that run ("1 2", "2", "" to disable). Round 1 starts
# at FIX1_HHMM (same Berlin day as the window start) while the sweep keeps running; round 2 starts
# once the sweep has ended. The morning summary waits for round 2.
FIXERS="${NIGHTLY_FIXERS-1 2}" # no colon: NIGHTLY_FIXERS="" really disables both rounds
FIXER_MODEL="${NIGHTLY_FIXER_MODEL:-claude-opus-5-5}"
FIX1_HHMM="${NIGHTLY_FIX1:-23:59}"
FIX1_MAX_SECONDS="${NIGHTLY_FIX1_MAX_SECONDS:-14400}"
FIX2_MAX_SECONDS="${NIGHTLY_FIX2_MAX_SECONDS:-10800}"
MAX_FIXES_PER_ROUND="${NIGHTLY_MAX_FIXES:-6}"
# The next night's sweep waits (at most this long past START) for the previous night's round 2
# and summary, which use the same checkout.
START_DEFER_SECONDS="${NIGHTLY_START_DEFER_SECONDS:-7200}"
# Checks and builds for the night branch, run before every game and to verify merged fixes.
BUILD_CMD="${NIGHTLY_BUILD_CMD:-cargo build --quiet -p ti4-server --bin server}"

# Each run plays until this round (or game over / first failure).
# Games end by themselves in round 9 (the public objectives run out, rule 81.2). Stopping at round 9
# cut every game off before that end-of-game path (final scoring, the VP tie-break), so the limit is
# round 10, which only a runaway game reaches: runs normally stop on game over.
STOP_ROUND="${NIGHTLY_STOP_ROUND:-10}"
MAX_DECISIONS="${NIGHTLY_MAX_DECISIONS:-20000}"
PLAYER_COUNTS="${NIGHTLY_PLAYER_COUNTS:-3 4}"
POLICIES="${NIGHTLY_POLICIES:-steer random}"
# Share of runs (percent) that start from a prepared state with fleets beside home systems and
# Mecatol Rex, so combat, casualties and the agenda phase show up early (see ti4-server preset.rs).
PRESET_PROBABILITY="${NIGHTLY_PRESET_PROBABILITY:-50}"
PRESET_NAME="${NIGHTLY_PRESET:-combat}"
# Free space (GB) needed on the report filesystem to start a run or a fixer round. Below it the loop
# waits (DISK_WAIT_SECONDS between checks) and gives the night up after MAX_DISK_WAITS checks.
# DF_CMD is a test hook (the tests put a stub `df` here).
MIN_FREE_GB="${NIGHTLY_MIN_FREE_GB:-10}"
DISK_WAIT_SECONDS="${NIGHTLY_DISK_WAIT_SECONDS:-120}"
MAX_DISK_WAITS="${NIGHTLY_MAX_DISK_WAITS:-30}"
DF_CMD="${NIGHTLY_DF_CMD:-df}"
# A fixer round that fails to start (no disk, no worktree) is retried after this long, up to this
# many attempts in total.
FIX_RETRY_SECONDS="${NIGHTLY_FIX_RETRY_SECONDS:-1800}"
FIX_MAX_ATTEMPTS="${NIGHTLY_FIX_MAX_ATTEMPTS:-2}"
# Do not start a new run with less time than this left before END (seconds).
MIN_RUN_SECONDS="${NIGHTLY_MIN_RUN_SECONDS:-1800}"
# Kill a run when its seat UI makes no progress for this long (seconds); the proctor reports it.
STALL_SECONDS="${NIGHTLY_STALL_SECONDS:-900}"

CLAUDE_BIN="${NIGHTLY_CLAUDE:-/root/.local/bin/claude}"
PROCTOR_MODEL="${NIGHTLY_PROCTOR_MODEL:-claude-sonnet-5-5}"
SUMMARY_MODEL="${NIGHTLY_SUMMARY_MODEL:-claude-opus-5-5}"

export PATH="/root/.local/bin:/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export HOME="${HOME:-/root}"

# The clock. Tests fake it: NIGHTLY_NOW_FILE (a file holding an epoch the test advances) wins over
# NIGHTLY_NOW (a fixed epoch); otherwise it is the real time.
now_epoch() {
  if [ -n "${NIGHTLY_NOW_FILE:-}" ] && [ -s "$NIGHTLY_NOW_FILE" ]; then
    cat "$NIGHTLY_NOW_FILE"
  elif [ -n "${NIGHTLY_NOW:-}" ]; then
    echo "$NIGHTLY_NOW"
  else
    date +%s
  fi
}

# Calendar helpers in Berlin time. Day arithmetic goes through the calendar (not "minus 86400"),
# so the two daylight-saving changes a year cannot move a window by an hour.
berlin_date() { TZ="$NIGHTLY_TZ" date -d "@${1:-$(now_epoch)}" +%F; }
shift_day() { TZ="$NIGHTLY_TZ" date -d "$1 12:00 $2 days" +%F; } # shift_day 2026-10-06 +1
day_epoch() { TZ="$NIGHTLY_TZ" date -d "$1 $2" +%s; }            # day_epoch 2026-10-06 09:00
hhmm_num() { printf '%s' "$1" | tr -d ':'; }

# Prints "<start-epoch> <end-epoch>" of the window that starts on Berlin date $1. The window
# crosses midnight when END is not after START.
window_for_night() {
  local start end_day="$1"
  start=$(day_epoch "$1" "$START_HHMM")
  [ "$(hhmm_num "$END_HHMM")" -le "$(hhmm_num "$START_HHMM")" ] && end_day=$(shift_day "$1" +1)
  echo "$start $(day_epoch "$end_day" "$END_HHMM")"
}

# Prints "<night-id> <start-epoch> <end-epoch>" for the window containing now, or the most recent
# window that already ended (used by the summary and the fix rounds).
current_window() {
  local now today night start end
  now=$(now_epoch)
  today=$(berlin_date "$now")
  read -r start end < <(window_for_night "$today")
  night="$today"
  if [ "$now" -lt "$start" ]; then
    night=$(shift_day "$today" -1)
    read -r start end < <(window_for_night "$night")
  fi
  echo "$night $start $end"
}

# Free space in whole GB on the filesystem holding the reports (the builds and games live there too).
free_gb() {
  mkdir -p "$REPORT_ROOT" 2>/dev/null || true
  $DF_CMD -Pk "$REPORT_ROOT" 2>/dev/null | awk 'NR == 2 { print int($4 / 1048576) }'
}
disk_ok() {
  local free
  free=$(free_gb)
  [ -n "$free" ] && [ "$free" -ge "$MIN_FREE_GB" ]
}

log() { echo "[$(TZ="$NIGHTLY_TZ" date -d "@$(now_epoch)" '+%F %T %Z')] $*"; }

# Kill every process that belongs to the run on <port>. All of a run's processes (Playwright,
# its web servers, browsers, the backend) inherit TI4_E2E_BACKEND_PORT from _run_inner.sh, which
# survives the new process groups/sessions Playwright creates and re-parenting to init.
kill_run_processes() {
  local port="$1" sig pids
  for sig in TERM KILL; do
    pids=$(grep -lsz "^TI4_E2E_BACKEND_PORT=$port\$" /proc/[0-9]*/environ 2>/dev/null \
      | cut -d/ -f3 | grep -vx -e "$$" -e "${BASHPID:-$$}" || true)
    [ -n "$pids" ] || return 0
    kill -"$sig" $pids 2>/dev/null || true
    [ "$sig" = TERM ] && sleep 5
  done
}
