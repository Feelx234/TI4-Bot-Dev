# GOLDEN45 (lane core-golden45, cut from core-integration-21 at 4a6cc5200)

## Problem

`host_create_golden` (bridge test, `tests/host_create.golden`, 1,405 rows) showed 45 rows differing on
core-integration-21: the 15 scenarios of three seats (`tactical_action`, `research_tech_skips`,
`production_batch`, `production_payment_batch`, `production_payment_autospend`, `space_combat`,
`ongoing_combat`, `ongoing_combat_four_views`, `ongoing_invasion_four_views`,
`ongoing_invasion_coexistence`, `ongoing_invasion_parley`, `endgame`, `score_objective_status`,
`score_objective_imperial`, `full_game`) on seeds 42, 1, 9. (The names in the report were scenario ids, not
presets. The two `late-game*` scenarios, all plain tables and all presets were fine.)

## Cause

Commit 31aae6296 "scenarios: real strategy card ids (the typed core refuses the aliases)", part of the D2-8d
wrapper lane, merged into int-21 with a8bb4da47 (core-d28f-movement with core-d28d-wrapper). It is NOT in
core-integration-20. The legacy scenarios dealt Sol/Hacan/Letnev the aliases `leadership`/`trade`/`warfare`
and, because the alias never matched, left the real cards in the unclaimed row and their trade-good piles in
`strategy_card_goods`. Since the commit, `base()` deals `pok1leadership`/`pok5trade`/`pok6warfare`, which
`retain` then really removes from the unclaimed row, and the piles are gone too (and the research / score
scenarios push `pok1leadership` back at the end of the row). Measured by dumping the state of `tactical_action`
seed 42 at 31aae6296^ (equal to the golden) and now: the only differences are the three hands, the unclaimed
row (3 cards fewer) and `strategy_card_goods` (3 piles fewer); the `research`/`score_imperial` rows differ in
the pushed card in the same way.

So the golden file is the legacy reference and the new state is the better one (a card cannot be both in a hand
and on the table; the typed core refuses the aliases). It is a legitimate, deliberate change, not a setup bug.
Int-21's report did not catch it: the golden test was not run after 31aae6296 arrived with the merge.
Int-20 was not bisected: its tip does not compile on its own (`ti4-engine-next` render/strategy.rs misses the
`Opt::LeakedSecret` arm, which core-gaps-19 brought into int-21), and the cause is already pinned.

## Fix (test only; the golden file is untouched, no row is loosened)

`crates/ti4-engine-core-bridge/tests/host_create_golden.rs`: for the 45 scenario rows of three seats the test now
turns the created state back into what the legacy scenario made (`as_the_legacy_scenario_dealt`: the three
dealt real ids named by their aliases, the three cards back in the unclaimed row in deck order, their piles back,
a pushed `pok1leadership` as the alias at the end) and compares THAT with the golden digest, exactly. It also
asserts that the real-id digest differs from the aliased one for each of the 45 rows (the rename is real and
counted: 45), so the exception cannot hide a no-op. All 1,103 other non-refused rows still equal the golden
exactly; the 302 Thunder's Edge rows are still asserted `ok` -> `err`.

Regression: the test itself. Without the helper it fails with the 45 differing rows (the state on int-21).

## Join notes

- Test file only; no engine, host, wire or rules change (RULES_VERSION 35, serde and generated TS unchanged).
- Shared CARGO_TARGET_DIR between two worktrees of the same repo is unsafe: cargo gave the same fingerprint
  to the workspace crates of both trees and reused stale artifacts. Use one target dir per tree.
- If the scenarios later deal cards differently again, update `as_the_legacy_scenario_dealt`; never the golden.
