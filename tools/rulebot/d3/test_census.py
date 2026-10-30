#!/usr/bin/env python3
"""Tests of census.py: python3 -I tools/rulebot/d3/test_census.py"""
import importlib.util
import os
import shutil
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("census", os.path.join(HERE, "census.py"))
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)

# Allowed distance from the statement-level numbers of docs/todo/D3-bots.md 1.2 (the script counts lines).
TOLERANCE = {"B": 2, "C": 4, "D": 8, "E": 2, "F": 2, "G": 2, "H": 2, "I": 2}


class Census(unittest.TestCase):
    def test_totals_reproduce_the_document(self):
        total, *_ = census.census()
        for c, tol in TOLERANCE.items():
            self.assertLessEqual(abs(total[c] - census.DOC[c]), tol, f"class {c}: {total[c]} vs doc {census.DOC[c]}")

    def test_committed_files_are_the_regenerated_text(self):
        total, by_file, by_fn, prod, test = census.census()
        self.assertEqual(open(os.path.join(HERE, "census.txt")).read(),
                         census.render_census(total, by_file, by_fn, prod, test))
        if shutil.which("zstd"):
            table, _, _ = census.subtype_table()
            self.assertEqual(open(os.path.join(HERE, "subtypes.tsv")).read(), table)

    @unittest.skipUnless(shutil.which("zstd"), "needs zstd")
    def test_every_corpus_subtype_is_classified_exactly_once(self):
        counts, _ = census.corpus_subtypes()
        rows = [r.split("\t")[0] for r in open(os.path.join(HERE, "subtypes.tsv")).read().splitlines()[1:]
                if r and not r.startswith("#")]
        self.assertEqual(len(rows), len(set(rows)), "a subtype text appears twice")
        self.assertFalse(set(counts) - set(rows), "corpus subtypes without a row")
        table = [r.split("\t") for r in open(os.path.join(HERE, "subtypes.tsv")).read().splitlines()[1:]
                 if r and not r.startswith("#")]
        for row in table:
            self.assertNotIn(row[4], ("", "?"), f"{row[0]}: no area")
            self.assertNotIn(row[5], ("", "?"), f"{row[0]}: no owner")

    def test_every_dispatched_subtype_has_a_site_or_predicate(self):
        sites, preds = census.bot_sites()
        # the dispatch of the router (lib.rs) is part of the table
        for text in ("activate_system", "movement_step", "draft_strategy_card", "research_technology", "cast_vote"):
            self.assertIn(text, sites)
        self.assertIn(("starts_with", "reaction_after_"), preds)
        self.assertIn(("ends_with", "_pick_technology"), preds)

    def test_a_mutated_tree_changes_the_totals(self):
        lines = ['    if subtype == Some("x") { let a = o.id == "done"; let b = PlayerId::new("p"); }']
        arms = census.subtype_arms(lines)
        self.assertEqual(arms, {})
        hits = {c: sum(1 for p in pats if p.search(lines[0])) for c, _, pats in census.COMPILED}
        self.assertTrue(hits["A"] and hits["B"] and hits["I"])


if __name__ == "__main__":
    unittest.main()
