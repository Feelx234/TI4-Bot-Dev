#!/usr/bin/env python3
"""Tests of zero_bail.py and its class table (run: python3 -I tools/engine_core/test_zero_bail.py)."""
import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import zero_bail as zb  # noqa: E402

SCRIPT = os.path.join(HERE, "zero_bail.py")
HEAD = "games 2 | areas on dual: [\"driver\"] | hashes equal: true\ndual: 10 calls, 10 comparisons, 0 differences\n"
GOOD_NATIVE = "native_play: 96 games | 96 finished with zero gaps | 0 gaps | 0 stopped at a known string bug (S1 to S5; excluded) | ZERO GAP\n"


def report(*rows):
    """A dual report with the given (label, kind, why, bailed) rows."""
    out = [HEAD, "  windows: 1 call sites\n"]
    for label, kind, why, n in rows:
        out.append(f"    {label}: 0 compared, {n} bailed\n")
        out.append(f"    {label} [{kind}: {why}]: 0 compared, {n} bailed\n")
    return "".join(out)


class Case(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def write(self, name, text):
        p = os.path.join(self.tmp.name, name)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w") as fh:
            fh.write(text)
        return p

    def bench(self, name, terminations=("victory",) * 3, problems=()):
        return self.write(f"{name}-dir/{name}.json", json.dumps({"games": [{"termination": t} for t in terminations], "problems": list(problems)}))

    def run_main(self, *args):
        return subprocess.run([sys.executable, "-I", SCRIPT, *args], capture_output=True, text=True)


class ClassTable(Case):
    def test_the_evidence_of_every_row_holds_in_the_tree(self):
        rows = zb.load_classes(zb.DEFAULT_CLASSES)
        self.assertGreater(len(rows), 30)
        self.assertEqual(zb.verify_evidence(rows, zb.DEFAULT_ROOT), [])

    def test_a_missing_file_or_token_is_reported(self):
        t = self.write("c.tsv", "census\td1::step\tx\twhy\tcrates/nope.rs::fn nothing ; tools/engine_core/zero_bail.py::no such token anywhere 12345\n")
        bad = zb.verify_evidence(zb.load_classes(t), zb.DEFAULT_ROOT)
        self.assertEqual(len(bad), 2, bad)
        self.assertIn("missing", bad[0] + bad[1])

    def test_a_row_needs_five_columns_a_known_class_and_evidence(self):
        for text in ["census\ta\tb\n", "weird\ta\tb\tr\te::t\n", "adapter\ta\tb\tr\t\n"]:
            with self.assertRaises(ValueError, msg=text):
                zb.load_classes(self.write("bad.tsv", text))

    def test_the_first_matching_row_wins_and_the_match_is_whole_string(self):
        rows = zb.load_classes(self.write("c.tsv", "census\td1::step\ttactical\tr\ta::b\nadapter\td1::step\t.*\tr\ta::b\n"))
        self.assertEqual(zb.classify("d1::step", "tactical", rows), ("census", 0))
        self.assertEqual(zb.classify("d1::step", "tactical2", rows), ("adapter", 1))
        self.assertEqual(zb.classify("xd1::step", "tactical", rows), ("unclassified", None))

    def test_a_bail_without_a_reason_is_silent_even_when_a_row_matches_everything(self):
        rows = zb.load_classes(self.write("c.tsv", "adapter\t.*\t.*\tr\ta::b\n"))
        self.assertEqual(zb.classify("game::close_vote", zb.NO_REASON, rows)[0], "silent")

    def test_the_causes_of_run_a_are_classified(self):
        rows = zb.load_classes(zb.DEFAULT_CLASSES)
        for label, why in [
            ("d1::step", "tactical"), ("d1::step", "ask.closing"), ("d1::step", "voting"), ("turn::hooks", "minister_of_war"),
            ("d1::reconcile", "announce_staged_ground_events"), ("tactical::activate", "Scanlink Drone Network held"),
            ("aftermath::fight_over", "Titans of Ul at the table (Coalescence)"), ("exploration::explore", "Titans' Terragenesis (after you explore a planet)"),
            ("tactical::activate", "nested (unported ability ability:*:aetherpassage:SYSTEM_ACTIVATED:after)"),
            ("invasion::finish_control_gain", "Integrated Economy (a triggered production nested in the chain)"),
        ]:
            cls, _ = zb.classify(label, why, rows)
            self.assertIn(cls, ("census", "adapter"), (label, why))

    def test_a_new_cause_is_not_classified(self):
        rows = zb.load_classes(zb.DEFAULT_CLASSES)
        self.assertEqual(zb.classify("tactical::activate", "Some New Technology held", rows)[0], "unclassified")


class Verdicts(Case):
    def test_native_zero_gap(self):
        ok, _ = zb.native_verdict(self.write("n.log", "game x\n" + GOOD_NATIVE))
        self.assertTrue(ok)

    def test_native_gaps_unfinished_games_and_a_dead_run_fail(self):
        for text in [
            "native_play: 96 games | 95 finished with zero gaps | 1 gaps | 0 stopped at a known string bug | 1 GAPS\n",
            "native_play: 96 games | 95 finished with zero gaps | 0 gaps | 0 stopped at a known string bug | ZERO GAP\n",
            "game x players 4 | rounds 3\n",
        ]:
            self.assertFalse(zb.native_verdict(self.write("n.log", text))[0], text)

    def test_bench_results_must_be_all_victories_without_problems(self):
        self.assertTrue(zb.bench_verdict(self.bench("bench"))[0])
        self.assertFalse(zb.bench_verdict(self.bench("bench", ("victory", "max_rounds")))[0])
        self.assertFalse(zb.bench_verdict(self.bench("bench", problems=[{"x": 1}]))[0])
        self.assertFalse(zb.bench_verdict(self.bench("bench", ()))[0])

    def test_a_log_of_a_dead_run_is_no_report(self):
        self.assertIsNotNone(zb.report_problem(self.write("d.log", "thread panicked at unmapped.rs:119\n")))
        self.assertIsNone(zb.report_problem(self.write("d.log", report())))


class Command(Case):
    def args(self, **kw):
        benches = []
        for n in kw.get("benches", ["bench", "bench-v1", "bench-v2", "bench-v3", "bench-v4", "bench-v5"]):
            benches += ["--bench", self.bench(n)]
        return ["--native", self.write("n.log", GOOD_NATIVE), *benches]

    def test_everything_classified_and_native_green_passes_in_enforce(self):
        r = self.run_main("--enforce", *self.args(), self.write("a.log", report(("d1::step", "bail", "tactical", 7))))
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("census 7", r.stdout)

    def test_an_unclassified_cause_fails_in_enforce_and_only_reports_otherwise(self):
        log = self.write("a.log", report(("tactical::activate", "bail", "Some New Technology held", 2)))
        r = self.run_main("--enforce", *self.args(), log)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("unclassified", r.stdout)
        self.assertEqual(self.run_main(*self.args(), log).returncode, 0)

    def test_a_gap_row_fails_in_enforce(self):
        classes = self.write("c.tsv", "gap\td1::step\ttactical\ta real hole\t\n")
        r = self.run_main("--enforce", "--classes", classes, "--no-evidence", *self.args(), self.write("a.log", report(("d1::step", "bail", "tactical", 1))))
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("gap", r.stdout)

    def test_a_silent_bail_fails_in_enforce(self):
        r = self.run_main("--enforce", *self.args(), self.write("a.log", report(("game::close_vote", "bail", zb.NO_REASON, 4))))
        self.assertEqual(r.returncode, 1, r.stdout)

    def test_the_lent_log_of_a_dead_step_fails_instead_of_counting_nothing(self):
        r = self.run_main("--enforce", *self.args(), self.write("a.log", report()), self.write("lent.log", "panic\n"))
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("not a dual report", r.stdout)

    def test_enforce_wants_the_native_log_and_six_bench_configs(self):
        log = self.write("a.log", report())
        self.assertEqual(self.run_main("--enforce", log).returncode, 1)
        r = self.run_main("--enforce", *self.args(benches=["bench", "bench-v1"]), log)
        self.assertEqual(r.returncode, 1)
        self.assertIn("6 wanted", r.stdout)

    def test_pending_rows_are_not_lends(self):
        r = self.run_main("--enforce", *self.args(), self.write("a.log", report(("d1::pending.open", "bail", "Tactical", 9))))
        self.assertEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=1)
