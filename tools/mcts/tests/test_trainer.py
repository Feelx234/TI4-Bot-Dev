"""Tests of the MCTS trainer: `VENV/bin/python -m unittest discover -s tools/mcts/tests` from the repository root.

A real shard written by `mcts_shards` is used when `TI4_MCTS_SHARD` names one; everything else runs on synthetic shards.
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

from ti4mcts import data, export, reference, synth  # noqa: E402
from ti4mcts.model import Config, Net, losses, parameter_count  # noqa: E402
from ti4mcts.schema import Schema  # noqa: E402

torch.set_num_threads(2)


class Tmp(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()


class SchemaAndData(Tmp):
    def test_gzip_and_plain_shards_give_the_same_batches(self):
        synth.write_synthetic(self.tmp / "gz", 300, seed=1, compress=True)
        synth.write_synthetic(self.tmp / "plain", 300, seed=1, compress=False)
        a, b = data.Shard(self.tmp / "gz"), data.Shard(self.tmp / "plain")
        rows = np.arange(0, 300, 7)
        ba, bb = data.make_batch(a, rows), data.make_batch(b, rows)
        for k in ba:
            self.assertTrue(torch.equal(ba[k], bb[k]), k)

    def test_batch_shapes_and_option_padding(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 200, seed=2))
        b = data.make_batch(s, np.arange(16))
        sc = s.schema
        self.assertEqual(tuple(b["sys"].shape), (16, sc.max_sys, sc.fs))
        self.assertEqual(tuple(b["adj"].shape), (16, sc.max_sys, sc.max_sys))
        self.assertTrue(torch.equal(b["adj"], b["adj"].transpose(1, 2)))
        ks = (np.asarray(s.a["opt_off"])[1:17] - np.asarray(s.a["opt_off"])[:16])
        self.assertEqual(b["opt"].shape[1], ks.max())
        self.assertEqual(b["opt_valid"].sum(dim=1).tolist(), ks.tolist())
        # Padding is never allowed.
        self.assertFalse((b["opt_mask"] & ~b["opt_valid"]).any())

    def test_held_out_games_are_disjoint(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 1000, seed=3))
        split = data.split_by_game([s], held_share=0.2, seed=0)
        keys = s.game_keys()
        train = set(keys[split.train[0][1]].tolist())
        held = set(keys[split.held[0][1]].tolist())
        self.assertTrue(held)
        self.assertFalse(train & held)
        self.assertEqual(len(split.train[0][1]) + len(split.held[0][1]), 1000)

    def test_a_wrong_feature_version_is_refused(self):
        d = synth.default_schema().to_dict()
        d["feature_version"] = 99
        with self.assertRaises(ValueError):
            Schema.from_dict(d).check()

    def test_a_real_shard_loads_when_given(self):
        root = os.environ.get("TI4_MCTS_SHARD")
        if not root:
            self.skipTest("TI4_MCTS_SHARD not set")
        s = data.Shard(root)
        b = data.make_batch(s, np.arange(min(64, len(s))))
        self.assertEqual(s.schema.fs, len(s.schema.names["sys"]))
        # Every record with a target has it on an allowed option, and the features are finite.
        has = b["has_pi"]
        self.assertTrue(((b["pi"] > 0) <= (b["opt_valid"] & b["opt_mask"])).all())
        self.assertTrue(torch.isfinite(b["sys"]).all() and torch.isfinite(b["opt"]).all())
        self.assertTrue(has.any())
        z = b["z"].sum(dim=1)
        self.assertTrue(torch.allclose(z, torch.ones_like(z), atol=1e-2))


class Model(Tmp):
    def test_forward_shapes_and_size(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 64, seed=4))
        net = Net(s.schema)
        out = net(data.make_batch(s, np.arange(8)))
        self.assertEqual(tuple(out["win"].shape), (8, 4))
        self.assertEqual(out["logits"].shape[0], 8)
        n = parameter_count(net)
        self.assertTrue(100_000 < n < 400_000, n)

    def test_padding_and_masked_slots_do_not_change_the_outputs(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 64, seed=5))
        net = Net(s.schema).eval()
        b = data.make_batch(s, np.arange(6))
        with torch.no_grad():
            out = net(b)
            # Garbage in the empty system slots and in the padded options changes nothing that is read.
            b2 = {k: v.clone() for k, v in b.items()}
            b2["sys"][~b2["sys_mask"]] = 7.0
            b2["opt"][~b2["opt_valid"]] = 3.0
            out2 = net(b2)
        self.assertTrue(torch.allclose(out["win"], out2["win"], atol=1e-5))
        v = b["opt_valid"]
        self.assertTrue(torch.allclose(out["logits"][v], out2["logits"][v], atol=1e-5))

    def test_imitation_learns_the_planted_teacher(self):
        root = synth.write_synthetic(self.tmp / "s", 6000, seed=6)
        s = data.Shard(root)
        torch.manual_seed(0)
        net = Net(s.schema, Config(d=32, ff=64, sys_blocks=1))
        opt = torch.optim.AdamW(net.parameters(), lr=2e-3)
        rng = np.random.default_rng(0)
        split = data.split_by_game([s], held_share=0.1, seed=0)
        first = None
        for epoch in range(3):
            for b in data.batches([s], split.train, 128, rng):
                l = losses(net(b), b)
                opt.zero_grad()
                l.total.backward()
                opt.step()
                first = first if first is not None else float(l.total.detach())
        net.eval()
        agree, n = 0.0, 0
        with torch.no_grad():
            for b in data.batches([s], split.held, 256):
                l = losses(net(b), b)
                agree += l.agree * l.agree_n
                n += l.agree_n
        from ti4mcts.train import chance_agreement

        chance = chance_agreement([s], split.held)
        self.assertGreater(agree / n, chance + 0.2, f"held-out agreement {agree / n:.3f} vs chance {chance:.3f}")


class Export(Tmp):
    def test_the_weights_round_trip_and_the_numpy_reference_matches(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 80, seed=7))
        torch.manual_seed(1)
        net = Net(s.schema).eval()
        # Not the zero init of the biases: give every parameter a value.
        with torch.no_grad():
            for p in net.parameters():
                p.add_(torch.randn_like(p) * 0.05)
        export.write_weights(net, self.tmp / "w.t4w")
        header, w = export.read_weights(self.tmp / "w.t4w")
        self.assertEqual(set(w), set(net.state_dict()))
        back = export.load_net(self.tmp / "w.t4w", s.schema)
        b = data.make_batch(s, np.arange(12))
        with torch.no_grad():
            o1, o2 = net(b), back(b)
        for k in ("win", "vp", "logits"):
            self.assertTrue(torch.equal(o1[k], o2[k]), k)
        for i in range(12):
            ref = reference.forward(header, w, reference.record(s, i))
            k = int(b["opt_valid"][i].sum())
            np.testing.assert_allclose(ref["win"], o1["win"][i].numpy(), atol=1e-4, rtol=0)
            np.testing.assert_allclose(ref["vp"], o1["vp"][i].numpy(), atol=1e-4, rtol=0)
            np.testing.assert_allclose(ref["logits"], o1["logits"][i, :k].numpy(), atol=1e-4, rtol=0)

    def test_the_fixtures_reproduce_their_outputs(self):
        s = data.Shard(synth.write_synthetic(self.tmp / "s", 120, seed=8))
        net = Net(s.schema).eval()
        m = export.write_fixtures(net, s, self.tmp / "fx", n=50)
        self.assertEqual(m["records"], 50)
        inputs = data.Shard(self.tmp / "fx" / "inputs")
        self.assertTrue((self.tmp / "fx" / "inputs" / "sys.npy").exists(), "plain .npy inputs for Rust")
        header, w = export.read_weights(self.tmp / "fx" / "weights.t4w")
        win = np.load(self.tmp / "fx" / "out_win.npy")
        logits = np.load(self.tmp / "fx" / "out_logits.npy")
        off = np.asarray(inputs.a["opt_off"])
        self.assertEqual(logits.shape[0], int(off[-1]))
        for i in range(0, 50, 7):
            ref = reference.forward(header, w, reference.record(inputs, i))
            np.testing.assert_allclose(ref["win"], win[i], atol=1e-4, rtol=0)
            np.testing.assert_allclose(ref["logits"], logits[off[i]:off[i + 1]], atol=1e-4, rtol=0)


if __name__ == "__main__":
    unittest.main()
