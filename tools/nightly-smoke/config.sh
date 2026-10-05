# Nightly UI smoke sweep configuration. Every value can be overridden from the environment,
# which is how the short test run shrinks the window (see README.md).

REPO="${NIGHTLY_REPO:-/root/TI4-Bot-Dev}"
NIGHTLY_DIR="$REPO/tools/nightly-smoke"
REPORT_ROOT="${NIGHTLY_REPORT_ROOT:-$REPO/nightly-reports}"

# Window in Berlin local time. The sweep starts at START and stops launching runs so that the
# last proctor can finish by END; the morning summary is written at END.
NIGHTLY_TZ="${NIGHTLY_TZ:-Europe/Berlin}"
START_HHMM="${NIGHTLY_START:-23:00}"
END_HHMM="${NIGHTLY_END:-08:00}"

# Each run plays until this round (or game over / first failure).
STOP_ROUND="${NIGHTLY_STOP_ROUND:-10}"
MAX_DECISIONS="${NIGHTLY_MAX_DECISIONS:-20000}"
PLAYER_COUNTS="${NIGHTLY_PLAYER_COUNTS:-3 4}"
POLICIES="${NIGHTLY_POLICIES:-steer random}"
# Do not start a new run with less time than this left before END (seconds).
MIN_RUN_SECONDS="${NIGHTLY_MIN_RUN_SECONDS:-1200}"
# Kill a run when its seat UI makes no progress for this long (seconds); the proctor reports it.
STALL_SECONDS="${NIGHTLY_STALL_SECONDS:-900}"

CLAUDE_BIN="${NIGHTLY_CLAUDE:-/root/.local/bin/claude}"
PROCTOR_MODEL="${NIGHTLY_PROCTOR_MODEL:-claude-haiku-4-5-20251001}"
SUMMARY_MODEL="${NIGHTLY_SUMMARY_MODEL:-claude-opus-5-5}"

export PATH="/root/.local/bin:/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export HOME="${HOME:-/root}"

# Seconds since epoch of the next/current HH:MM in Berlin time (today's date in Berlin).
berlin_epoch() { TZ="$NIGHTLY_TZ" date -d "$(TZ="$NIGHTLY_TZ" date +%F) $1" +%s; }
now_epoch() { date +%s; }

# The night a moment belongs to is named after the Berlin date on which its window started.
# Prints "<night-id> <start-epoch> <end-epoch>" for the window containing now, or the most
# recent window that already ended (used by the summary).
current_window() {
  local now start end
  now=$(now_epoch)
  start=$(berlin_epoch "$START_HHMM")
  end=$(berlin_epoch "$END_HHMM")
  # The window crosses midnight when END <= START.
  if [ "$end" -le "$start" ]; then
    if [ "$now" -lt "$end" ]; then
      start=$((start - 86400))
    else
      end=$((end + 86400))
    fi
  fi
  # Before today's start (and not inside last night's window): report last night.
  if [ "$now" -lt "$start" ]; then
    start=$((start - 86400))
    end=$((end - 86400))
  fi
  echo "$(TZ="$NIGHTLY_TZ" date -d "@$start" +%F) $start $end"
}

log() { echo "[$(TZ="$NIGHTLY_TZ" date '+%F %T %Z')] $*"; }

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
