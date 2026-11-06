#!/usr/bin/env bash
# The CPU-only PyTorch venv of the MCTS trainer (about 1 GB), outside the repository.
#
#   tools/mcts/setup_venv.sh [VENV]          default /root/mcts-venv
#   VENV/bin/python -m unittest discover -s tools/mcts/tests      (from the repository root)
#
# The system Python has no ensurepip: the venv is made without pip and pip is bootstrapped with get-pip.py.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
venv="${1:-/root/mcts-venv}"
tmp="${TMPDIR:-/root/deploytmp}/mcts-getpip"
if [[ ! -x "$venv/bin/python" ]]; then
  python3 -m venv --without-pip "$venv"
fi
if ! "$venv/bin/python" -m pip --version >/dev/null 2>&1; then
  mkdir -p "$tmp"
  curl -sS -o "$tmp/get-pip.py" https://bootstrap.pypa.io/get-pip.py
  "$venv/bin/python" -I "$tmp/get-pip.py" -q
fi
TMPDIR="${TMPDIR:-/root/deploytmp}" "$venv/bin/python" -m pip install -q --no-cache-dir -r "$here/requirements.txt"
"$venv/bin/python" -c "import torch, numpy; print('torch', torch.__version__, 'numpy', numpy.__version__)"
