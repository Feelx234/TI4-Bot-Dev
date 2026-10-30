# D4A2 resume (lane A2 of D4: D4-2d combat windows, D4-2e invasion windows, lane A1 weak spots)

Branch `core-d4a2` (worktree `.claude/worktrees/core-d4a2`), cut from `core-integration-6` @ `1cd71a13`; `core-integration-6` @ `c519216e` was merged in
(no conflicts). Only test code, the kit's `kernel.rs` (see D4A2-design-01) and docs changed; **no production code**.

## What was run, what was not

* Run (cheap): `cargo test -p ti4-engine-core-tests` in profile `lockstep` with debug assertions and overflow checks ON for the three core crates
  (as `tools/engine_core/d4/run_core_tests.sh`), CARGO_BUILD_JOBS=2, one private target dir; **861 tests pass, 0 fail** (lane A1: 611). 250 new tests.
* Run (cheap): single-file mutation checks (`tools/engine_core/mutants.sh`, `TEST_CRATE=ti4-engine-core-tests`) over the 31 files of `rules/combat` and
  `rules/invasion` by a Haiku runner on commit `182f8826` (the first combat/invasion tests, before the survivor-driven tests below).
* **Mutation ratchet deferred to end of D1** (user directive): no `mutation_ratchet.sh`, no module-level run, no floor raised or written
  (`tools/engine_core/mutation_floor.json` untouched; it has no `core/rules/combat` / `core/rules/invasion` rows yet).
* **Not run** (user directive, nothing heavy until the end of D1): `verify_package`/gate, golden or wide dual runs, benches, soak, the ratchet. No
  mutation run of the files changed AFTER `182f8826` (the survivor-driven tests and the weak-spot tests have no "after" number), no clippy run.

## Tests added (`crates/ti4-engine-core-tests/tests/`)

| file | tests | covers |
|---|---:|---|
| `combat_small.rs` | 24 | AssignHits, SustainOffer, SustainCommander (Farran), BeginRound, Retreat |
| `combat_barrage.rs` | 17 | barrage plan and step (seeded dice), DestroyFighters, Raid Formation, Assault Cannon |
| `combat_end.rs` | 11 | end (winner/finished), end_report, feats |
| `combat_cannon.rs` | 17 | cannon plan (Plasma, Antimass, Disable), cannon guns, SpaceCannonOffense, Absorb |
| `combat_flow.rs` | 12 | Flow transitions, EndAnnounce |
| `combat_roll.rs` | 13 | RollStep: ordering, queue, Munitions, Crown of Thalnos, Wrath, War Funding, Heart of Ixth (pinned) |
| `combat_machine.rs` | 9 | the composed combat settle by settle (whole fights, retreat, Rout/Intercept, barrage feat pause, bails) |
| `combat_extra.rs` | 11 | survivor-driven: barrage dice records/window conditions, Argent flagship bar, attachment cannons, Graviton, feat, Naaz Maximum |
| `invasion_rules.rs` | 17 | bombardable/bombard_plan, landing, commit options, ground hits, pools, custodians |
| `invasion_windows.rs` | 31 | GroundHits, AbsorbGround, CommitGround/LandOne, Custodians, FightRound/NextCombat/BombardTarget, defense, rounds, roll, tail, Harrow, Bombardment |
| `invasion_extra.rs` | 15 | survivor-driven: multi-planet/group bombardment, feat conditions, coexistence in Advance, landing flags, deploy, hits details |
| `action_card_effects.rs` | 22 | 22 action cards against their printed text (lane A1 had 5) |
| `query_trade_deals.rs` | 19 | deal list, prices, offers, Pride of Kenara |
| `query_tech_edges.rs` | 5 | NoHooks, breakthrough synergy, satisfies boundaries, war sun law, Research Team waivers |
| `agenda_table_effects2.rs` | 9 | Table::of mapping, Rearmament, Plowshares, Core Mining, DMZ, Clandestine, wormhole effects, Travel Ban, Minister |
| `agenda_questions.rs` | 4 | Seed of an Empire, Homeland Defense Act |
| `production_window2.rs` | 9 | limits, discount, free use, credit, Winnu agent, filters |
| `research2.rs` | 5 | specialty exhaustion for prerequisites, waivers, Research Agreement |

Snapshots: `tests/snapshots/{combat_small,combat_barrage}/`.

## First mutation numbers (commit `182f8826`, files of `rules/combat` and `rules/invasion`; killed / survived)

| file | killed | survived | killed % |
|---|---:|---:|---:|
| combat/assign.rs | 20 | 12 | 62% |
| combat/begin_round.rs | 11 | 2 | 85% |
| combat/sustain_offer.rs | 11 | 5 | 69% |
| combat/sustain_commander.rs | 12 | 2 | 86% |
| combat/retreat.rs | 17 | 5 | 77% |
| combat/barrage.rs | 26 | 18 | 59% |
| combat/destroy_fighters.rs | 7 | 3 | 70% |
| combat/raid_formation.rs | 27 | 2 | 93% |
| combat/assault_cannon.rs | 16 | 4 | 80% |
| combat/cannon_guns.rs | 19 | 7 | 73% |
| combat/end.rs | 15 | 0 | 100% |
| combat/end_announce.rs | 7 | 2 | 78% |
| combat/feats.rs | 39 | 2 | 95% |
| combat/flow.rs | 64 | 6 | 91% |
| combat/space_cannon_offense.rs | 49 | 9 | 84% |
| invasion/absorb.rs | 21 | 5 | 81% |
| invasion/advance.rs | 13 | 5 | 72% |
| invasion/begin.rs | 10 | 2 | 83% |
| invasion/bombard.rs | 39 | 22 | 64% |
| invasion/commit.rs | 25 | 16 | 61% |
| invasion/defense.rs | 40 | 17 | 70% |
| invasion/harrow.rs | 12 | 8 | 60% |
| invasion/hits.rs | 33 | 7 | 82% |
| invasion/roll.rs | 18 | 12 | 60% |
| invasion/round.rs | 4 | 5 | 44% |
| invasion/stages.rs | 42 | 2 | 95% |
| invasion/tail.rs | 13 | 8 | 62% |
| invasion/mod.rs | 129 | 40 | 76% |
Totals of the 31 runs: 739 killed, 228 survived. The runner kept only the last 60 lines of each log, so the survivor lists of `combat/roll.rs` and
`combat/machine.rs` (and parts of other long ones) are incomplete; their summary lines are missing from the table above (28 of 31 rows). Most survivors
left are the bodies of `ask_of`/`answer` of windows that never ask (unreachable through the driver: it refuses a pick when no question is open) and
literal `0 -> 1` in initialisers. The real gaps (barrage dice records, cannon gates, Graviton, feats, bombardment cursor, landing flags, ...) got
the `combat_extra.rs` / `invasion_extra.rs` tests.

## Second mutation round (commit `33e5dce1`, with all tests of this lane; killed / survived)

Weak-spot files run with the A1 + A2 tests of their area. Same runner, nothing raised or written to the floors.

| file | killed | survived | killed % |
|---|---:|---:|---:|
| query/tech.rs | 172 | 20 | 90% |
| query/trade_deals.rs | 135 | 60 | 69% |
| rules/agenda/table_effects.rs | 165 | 41 | 80% |
| rules/combat/assign.rs | 27 | 5 | 84% |
| rules/combat/barrage.rs | 33 | 11 | 75% |
| rules/combat/cannon_guns.rs | 23 | 3 | 88% |
| rules/combat/machine.rs | 119 | 103 | 54% |
| rules/combat/mod.rs | 124 | 248 | 33% |
| rules/combat/roll.rs | 142 | 87 | 62% |
| rules/combat/space_cannon_offense.rs | 55 | 3 | 95% |
| rules/invasion/bombard.rs | 53 | 8 | 87% |
| rules/invasion/commit.rs | 36 | 5 | 88% |
| rules/invasion/defense.rs | 42 | 15 | 74% |
| rules/invasion/harrow.rs | 13 | 7 | 65% |
| rules/invasion/round.rs | 4 | 5 | 44% |
| rules/invasion/tail.rs | 15 | 6 | 71% |
| rules/production/window.rs | 63 | 94 | 40% |

Reading: combat files that got survivor-driven tests moved up (barrage 59 -> 75, cannon_guns 73 -> 88, space_cannon_offense 84 -> 95, bombard 64 -> 87,
commit 61 -> 88, defense 70 -> 74); `combat/mod.rs` (33%, 248 survivors: the sustain/roll/fleet helper functions), `combat/machine.rs` (54%), `combat/roll.rs`
(62%), `invasion/round.rs`, `production/window.rs` (40%), `trade_deals.rs` (69%) stay the next work. Full survivor lists: `/root/deploytmp/core-d4a2/mut2-*.txt`
(deleted with the scratch dir; re-run `mutants.sh` per file).

## Findings (docs/ENGINE_KNOWN_ISSUES.md rows `D4A2-*`)

* D4A2-found-01 Tactical Bombardment ignores the BOMBARDMENT requirement of its text (both engines). Pinned.
* D4A2-design-01 the kit's `kernel_all()` lacked `register_core` and `bridge_legacy`: fixed in the kit.
* D4A2-found-02 `RollStep::question` skips a War Funding offer when the last-rolling side has no dice (unreachable). Not fixed.
* D4A2-info-01/02 Heart of Ixth pinned (A1); questions whose options their own window refuses.
No core defect was fixed (no behaviour change allowed in D4).

## How to resume

1. `git log --oneline core-integration-6..core-d4a2`; run `tools/engine_core/d4/run_core_tests.sh $(git rev-parse HEAD)` (or the cargo line above).
2. Still weak (not touched): `query/objectives.rs` (302 survivors in the A1 run: `SeatFacts::of`, `secret_row`, same-trait, notable systems, bought progress; needs
   `Pos`/`SeatFacts`/`Terms` fixtures), `query/reach.rs`, `query/transit.rs`, `query/trade.rs`, agenda `window.rs`/`mod.rs`, production `mod.rs`
   (`build_for`, `post_build`, `triad_value`), `strategy_cards.rs` Leadership/Construction, `offers.rs`, the other ~60 ported action cards (reaction
   cards need the reaction-window harness), the survivors of `combat/roll.rs` and `combat/machine.rs` (re-run without `tail`).
3. At the end of D1: `MODE=baseline EXTRA_TEST_CRATES=ti4-engine-core-tests tools/engine_core/mutation_ratchet.sh` over the modules, add `core/rules/combat` and
   `core/rules/invasion` rows to `tools/engine_core/d4/mutation_modules.tsv`, then raise the floors.
4. Scratch: `/root/deploytmp/core-d4a2` (target dir, logs, mutation summary) is deleted when this lane is finished.
