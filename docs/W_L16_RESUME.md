# W lane 16 resume note, branch `core-w-l16` (from `core-w-l15` @ `ba43ce6a`, merged with `core-m5-aftermath` @ `b15c8773`)

Task: the Left list of `docs/W_L15_RESUME.md`: (1) Yin Devotion in the composed combat machine, (2) the bombardment and space cannon reroll windows of the Jol-Nar
commander, (3) the missing core judges of the eight cards whose hosts are other windows. Pure refactor: `RULES_VERSION`, serde and fixtures are unchanged. Rows for
`docs/ENGINE_KNOWN_ISSUES.md` are in its section "W lane 16" (W16-design-01 to 04, W16-find-01). Never pushed; the live branch is untouched.

## Owners

* Lane M5 (`core-m5-aftermath`) is editing the combat machine (Van Hauge drain points, the EventScoring window, the combat live arm). Its first tip was merged in the middle of this
  lane and its final tip (`b15c8773`) at the end; the conflicts of the first merge (the `Unknown` enum, `sustain_offer.rs` doc, `cannon_dual.rs` doc, `reactions_core::set_table_codes`,
  `combat.rs` tests, two docs) were resolved keeping both sides, and `ReflectiveShielding` stays out of `Unknown` (lane 11 ported it). The merge needed two fixes in M5's
  files: `driver::windows::combat::env_of` sets `show_neutral: false` (the live path has no string engine to compare with) and `driver::live`'s test provider sets `shared`.
* Item 1 (Devotion) is M5's code: its `Machine::pending_choice` asks the producer of a `producer_assigns` pend (see `docs/M5_RESUME.md`). This lane owns the proof: Yin in the rotation of
  the neutral-side scenario and the producer-asks lockstep test.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!`/`dual::write` + lockstep test; bail-as-failure scenario tests)

| item | where | tests |
|---|---|---|
| Yin Devotion in the composed machine | `rules/combat/machine.rs` (M5), `combat.rs` tests | `a_neutral_side_fights_in_the_core_for_every_faction_across_from_it` (22 -> 23 factions, Yin included, single windows and composed, neutral or not, no bail), `a_devotion_hit_is_asked_of_its_producer_in_the_composed_machine_in_lockstep` (30 seeds x single/composed, the Yin seat as attacker and defender, victims with dreadnoughts, carriers and cruisers: the producer's casualty question is compared with the string's, composed `bailed == 0`); the pinning test `a_yin_seat_that_uses_devotion_keeps_the_composed_machine_on_the_string_and_says_why` is gone |
| The Jol-Nar commander in the reroll window of a bombardment | `kernel/commander_walk.rs` (`Walk`), `rules/invasion/bombard_roll.rs` (`Facts::{commander, shown}`, stages `Walking`, `Rolled`), `dual_window::bombard_rolled` (+ `roll_details`), gate removed from `bombard_rolled_why` | core `w16_commander_hosts` (3), engine `invasion::tests::the_jolnar_commander_rerolls_the_dice_of_a_bombardment_in_the_core_in_lockstep` (14 seeds, three dreadnoughts, window compared and not bailed on every seed, with and without a Scramble Frequency), the gate test is now `a_bombardment_of_a_heart_of_ixth_or_irreproducible_dice_...` |
| The Jol-Nar commander in the space cannon offense (gunners with hits, also through the aftermath head) | `rules/combat/space_cannon_offense.rs` (`Facts::{jolnar, unit_texts, planet_texts}`, stage `Rerolling`, staged entries carry the unit text), `cannon_dual.rs` (`reroll_texts`: only interned when a commander is in play), `driver/windows/tactical/facts.rs` (live facts) | core `w16_commander_hosts` (2), engine `cannon_prelude_tests::{a_jolnar_commander_rerolls_the_guns_in_the_core_in_lockstep, the_jolnar_commander_of_two_gunners_and_a_scramble_frequency_agree_in_lockstep, a_head_with_the_jolnar_commander_asks_the_gunner_in_the_core_in_lockstep}` (the Heart of Ixth keeps its gate: `a_heart_of_ixth_keeps_the_offense_on_the_string`) |
| The Jol-Nar commander in the native invasion window (the counted bail `Unported::JolNarCommanderReroll` of `docs/M3_RESUME.md` is gone) | `driver/windows/invasion/ops.rs` (`begin_walk`, `answer_walk`, `walk_of`), `state/invasion.rs` (`IBombard::{walked, walk: IWalk}`), `InvasionCodes::shown_units` (filled by `invasion_live::fill_codes`) | core `invasion_composer::the_commander_of_the_invader_is_asked_about_each_die_of_the_bombardment_and_the_walk_survives_a_clone` (12 seeds, a clone at every question), engine `invasion_live::tests::the_jolnar_commander_rerolls_the_dice_of_a_bombardment_natively` (8 defender factions, the questions, options and context are the string window's) |
| Harness Energy judge | `rules/action_cards/mod.rs` `system_is_anomaly`, `reaction_can_matter_with` (+ `Hooks`) | core `w16_judges` (every system of the catalogue against its flags, the rift hook), engine `action_cards_check::tests::the_judges_of_harness_energy_and_infiltrate_agree_with_the_strings_over_many_positions` (40 random boards, every occupied system x every seat x every actor, > 3000 comparisons) |
| Infiltrate judge | `rules/action_cards/mod.rs` `infiltrate_swaps` (shared with `Fx::Infiltrate`), `Window::planet` | the same two files (swaps counted as they go, a box run down, the form bar, rules versions 0/1/23, the gainer only, no planet named) |
| Bribery potential-goods judge | `kernel/fx.rs` `potential_goods` (also `production::supply::potential_goods`, the agenda window's), `reaction_slot::can_matter` | core `w16_judges::the_potential_goods_...`, `reaction_slot::tests::a_condition_the_core_cannot_answer_is_unknown_not_false`, engine `action_cards_check::tests::the_potential_goods_of_a_seat_are_the_strings_over_many_positions` (40 random tables, Bribery payable on the core goods iff the string says it is, rules version 22/23/24) |
| Lie in Wait judge | `query/trade.rs` `neighbours_who_transacted` (existed) | engine `transactions::tests::the_neighbours_who_transacted_are_the_core_judges_answer_over_many_positions` (60 hub positions of 4 to 6 seats with random transactions, run under `trade=dual`: every call compared with the string's) |
| Play facts of the live loop | `ti4-engine/src/live/plays.rs` | filled `SeatPlayMore` and `SharedPlay` from the core (the NOTE of M5 for the merge of lane 11) |

## Left (precise reasons)

* The eight cards themselves (Upgrade, Decoy Operation, Harness Energy, Infiltrate, Bribery, Lie in Wait, Crash Landing, Experimental Battlestation): their hosts are windows of
  `core-m6-tactical` (activation, movement), the invasion control step, the vote and the transaction windows; `Fx::nested` stays false for them. With the judges done, a seat that holds
  Harness Energy / Infiltrate / Bribery is no longer a counted bail at the window (`Unknown::HarnessOrInfiltrate` and `Unknown::BriberyGoods` are deleted): the card is offered by
  `reaction_slot` and, if played, the play declines at the registered effect (`Unknown::UnportedCard`) until a host flips `nested`. Lie in Wait still reads the map (`needs_map`).
* The live tactical window routes the head's questions only in the Enforcing stage: a Jol-Nar commander's question about the guns' dice in the live path is the counted bail
  `TacticalGap::CannonHits` (the facts name the commander, so it is never skipped silently); M6 owns the routing. The native invasion asks its walk with the staged entry's own code
  as label unless `InvasionCodes::shown_units` is filled (the engine fills it); the `dice`/`current` display details of those questions are not added by the live adapter
  (`live::details::complete` has no `RerollDie` case) and are not compared by `invasion_live` (it compares prompt, options, context).
* The Heart of Ixth stays a gate (`bombard_rolled_why`, `cannon_dual::unported`, the barrage): Thunder's Edge.
* Lane M5 may still move: this lane merged its tip `b15c8773`; the merge is a plain merge commit.

## Checks (cheap only; private target `/root/deploytmp/core-w-l16/target`, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, one cargo at a time)

Run (all green on the last code commit): `cargo test -p ti4-engine --lib` whole (4960 passed, 4 ignored; W15 had 4896, the lanes merged from M5/M3 and this lane's tests are the rest) normal and under
`TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (4960 passed); `cargo test -p ti4-engine-core --lib` (466), `-p ti4-engine-core-factions --lib` (129), `-p ti4-engine-core-tests` (every binary, including
the new `w16_commander_hosts` (5) and `w16_judges` (6), `invasion_composer` (7)); `tools/engine_core/check_layers.py core` (0 violations; the first version of the walk and of `potential_goods`
had three, fixed by moving them to `kernel/`); `tools/engine_core/lint_core.sh` (0 hits).

Regression tests that fail on the pre-W16 code: the neutral-side rotation with Yin (the pinned `Bail::Unported`), `a_devotion_hit_is_asked_of_its_producer_...` (composed `bailed == 0`), every Jol-Nar
lockstep above (the pre-W16 gates leave `compared == 0` and the windows on the string; the core tests do not compile without the new facts), the judges' lockstep (`None`, a counted bail,
before; `judged > 3000` now), the Lie in Wait and Bribery lockstep (new comparisons of existing core code). The first runs of the new tests found: the Jol-Nar PDS roll has a third die in
the engine's test position (the assertion on the number of questions was loosened, the lockstep stays exact), the composed machine's `show_neutral` missing from M5's `env_of`, and the stub of
`w15_scramble_hosts` (W16-find-01).

Not run (a full zero-bail playthrough is not possible yet): golden / wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy.
