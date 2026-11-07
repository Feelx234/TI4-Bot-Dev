"""Write the small parity fixture of the Rust forward pass (`crates/ti4-mcts-net/tests/fixtures/tiny`).

A small model (d 16, 2 heads) with random weights (every parameter perturbed, so no path is zero) on 24 records of a real shard:

    /root/mcts-venv/bin/python tools/mcts/make_tiny_fixture.py SHARD_DIR crates/ti4-mcts-net/tests/fixtures/tiny
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import torch  # noqa: E402

from ti4mcts.data import Shard  # noqa: E402
from ti4mcts.export import write_fixtures  # noqa: E402
from ti4mcts.model import Config, Net  # noqa: E402


def main() -> None:
    shard, out = Shard(sys.argv[1]), Path(sys.argv[2])
    torch.manual_seed(7)
    net = Net(shard.schema, Config(d=16, heads=2, ff=32, sys_blocks=2, joint_blocks=1, fac_dim=4, obj_dim=4, kind_dim=4, head_hidden=16))
    with torch.no_grad():
        for p in net.parameters():
            p.add_(torch.randn_like(p) * 0.1)
    m = write_fixtures(net, shard, out, n=24, seed=3)
    print(m)


if __name__ == "__main__":
    main()
