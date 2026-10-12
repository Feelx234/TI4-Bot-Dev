#!/usr/bin/env bash
# Build and release the public playtest server. Run as a normal user with sudo rights on the
# server, from a checkout of the repo (it is NOT run by CI and nothing here is installed for you).
#
#   deploy/public/deploy.sh            # pull main, build, release, restart
#   deploy/public/deploy.sh rollback   # point `current` back at the previous release
#
# Layout:  /opt/ti4-public/releases/<UTC timestamp>/{server,dist/,COMMIT}
#          /opt/ti4-public/current  -> releases/<newest>        (atomic symlink swap)
#          /opt/ti4-public/previous -> releases/<one before>    (rollback target)
set -euo pipefail

ROOT="${TI4_PUBLIC_ROOT:-/opt/ti4-public}"
SERVICE="${TI4_PUBLIC_SERVICE:-ti4-public}"
BRANCH="${TI4_PUBLIC_BRANCH:-main}"
KEEP="${TI4_PUBLIC_KEEP_RELEASES:-4}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# Own target dir so this never fights with agent builds in the shared checkout.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/ti4-public-target}"

say() { printf '\n== %s\n' "$*"; }

swap_current() { # $1 = release dir
  local new="$1"
  sudo ln -sfn "$new" "$ROOT/current.new"
  sudo mv -T "$ROOT/current.new" "$ROOT/current"
}

health_check() {
  local port
  port="$(sudo grep -E '^PORT=' /etc/ti4-public/env | tail -1 | cut -d= -f2)"
  port="${port:-18200}"
  for _ in $(seq 1 90); do
    if curl -fsS "http://127.0.0.1:${port}/health" >/dev/null 2>&1; then
      curl -fsS "http://127.0.0.1:${port}/health"; echo
      return 0
    fi
    sleep 2
  done
  return 1
}

if [[ "${1:-}" == "rollback" ]]; then
  [[ -L "$ROOT/previous" ]] || { echo "no previous release to roll back to" >&2; exit 1; }
  target="$(readlink -f "$ROOT/previous")"
  say "Rolling back to $target"
  current="$(readlink -f "$ROOT/current")"
  swap_current "$target"
  sudo ln -sfn "$current" "$ROOT/previous"
  sudo systemctl restart "$SERVICE"
  health_check && echo "rolled back" || { echo "health check failed after rollback" >&2; exit 1; }
  exit 0
fi

cd "$REPO"
say "Updating $BRANCH"
git fetch origin "$BRANCH"
git checkout "$BRANCH"
git pull --ff-only origin "$BRANCH"
COMMIT="$(git rev-parse --short=8 HEAD)"
export TI4_BUILD_COMMIT="$COMMIT"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RELEASE="$ROOT/releases/${STAMP}-${COMMIT}"

say "Building the web client ($COMMIT)"
( cd web && npm ci && npm run build )

say "Building the release server"
cargo build --release -p ti4-server --bin server

say "Installing $RELEASE"
sudo install -d -m 755 "$RELEASE"
sudo install -m 755 "$CARGO_TARGET_DIR/release/server" "$RELEASE/server"
sudo cp -r web/dist "$RELEASE/dist"
echo "$COMMIT" | sudo tee "$RELEASE/COMMIT" >/dev/null
sudo chown -R root:root "$RELEASE"

previous=""
[[ -L "$ROOT/current" ]] && previous="$(readlink -f "$ROOT/current")"

say "Switching to the new release and restarting"
swap_current "$RELEASE"
[[ -n "$previous" ]] && sudo ln -sfn "$previous" "$ROOT/previous"
sudo systemctl restart "$SERVICE"

say "Health check"
if health_check; then
  echo "release $COMMIT is live"
else
  echo "health check FAILED; rolling back" >&2
  if [[ -n "$previous" ]]; then
    swap_current "$previous"
    sudo systemctl restart "$SERVICE"
  fi
  exit 1
fi

say "Pruning old releases (keeping $KEEP)"
# shellcheck disable=SC2012
ls -1dt "$ROOT"/releases/*/ | tail -n +"$((KEEP + 1))" | while read -r old; do
  case "$(readlink -f "$old")" in
    "$(readlink -f "$ROOT/current")"|"$(readlink -f "$ROOT/previous" 2>/dev/null || true)") ;;
    *) sudo rm -rf "$old" ;;
  esac
done
