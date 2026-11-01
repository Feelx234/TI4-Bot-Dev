# Lane K (`core-k-actioncards`): action cards played natively (resume note, 2026-11-01)

Branch `core-k-actioncards`, cut from `core-integration-14` (`4766eafcc`). Never pushed; `nightly-fixes-2026-10-26` untouched. Pure refactor: RULES_VERSION, serde, fixtures and fingerprints of the string engine unchanged.
Read `docs/INTEGRATION14.md` sections 5, 6, 8 and `docs/KM2_RESUME.md` (the menu dispatch of the action card) first.

## 1. What was wrong (the whole first gap `Menu(ActionCard)` was ONE cause)

The dispatch of the play, the window it opens, the announce/cancel chain and the reaction windows were already native. What stopped 38 of 96 (172 of 288) games was
`driver::menu_cards::action_cards`: **Signal Jamming** in a hand made the whole action menu the counted gap `MenuGap::ActionCard` (the core answered "the map's blockade rules" with a bail). Every hand dealt it at some point.
The gap was reported by the arm that ENDED the step (`secondary`, `ask.action_menu`), so the matrix looked like "an action card played in a secondary". Nothing is played in a secondary; the menu of the next turn could not be built.

Causes found and fixed, by the order the sweeps exposed them (each has a regression test that fails on the pre-fix code):

| # | cause | where | fix | guard |
|---|---|---|---|---|
| K1 | Signal Jamming on the menu: `MenuGap::ActionCard` | `driver/menu_cards.rs::action_cards` | `rules::action_cards::effects::jam_systems` (the systems of `Fx::Jam`'s first question, moved out of the window) answers `is_playable` (a jammable system and a rival); a position without the Map slice stays the counted gap | core `action_dispatch`: `signal_jamming_is_listed_when_a_ship_has_a_system_in_reach_and_a_rival_exists`, `..._without_a_map_is_the_counted_gap`; bridge `menu_sources` now asserts `gaps == 0` for the cards and that Signal Jamming was compared OFFERED (90 seats) and WITHHELD (new variant "signal jamming, no ships") against the string |
| K2 | Summit with a command token in a Mahact fleet pool: `Play::of` declined (`Unknown::PlayFacts`, arm `phase`) | `rules/action_cards/effects.rs` | the bail was obsolete: `fx::tokens_in_reinforcements` and `Env::mahact_held` count the held tokens; Summit asks both pool questions (the string does) and gains only what is left | core `action_card_effects3::summit_counts_the_tokens_a_mahact_fleet_pool_holds_out_of_the_reinforcements` |
| K3 | Intercept and Courageous To The End declined at any table with a Naaz or Nekro seat (`PlayFacts`, arm `aftermath`) | `effects.rs`; `rules/combat/mod.rs` | `ships_of`, `planet_combatants`, `planetary_maximum_participates`, `planets_of_sorted` moved to `kernel::read` (L3; "an L4 area does not import another"), re-exported from `rules::combat`; the two cards use `ships_of` (units that fight from a planet are combatants) | core `action_card_effects2::intercept_counts_the_ground_forces_a_nekro_alastor_chose_as_a_combatant`, `ships_of_adds_the_units_that_fight_from_a_planet`; core `action_card_effects::courageous_to_the_end_finds_a_nekro_opponent_that_fights_from_its_planet`; engine lockstep (dual, string vs core, 12 seeds) `courageous_to_the_end_finds_a_nekro_opponent_that_fights_from_its_planet_in_lockstep`, `intercept_at_a_table_with_a_nekro_seat_is_the_cores_and_counts_the_alastors_choice` |
| K4 | Refit Troops for a Naaz seat declined (`PlayFacts`, arm `ask.action_menu`) | `live/plays.rs`, `reactions_core.rs` (the Naaz mech was "no unit"); `effects.rs` | the Naaz mech is `Unit::Eidolon`; each placement stages `NAAZ_MECH_PLACED` (`fx::stage_naaz_mech_placed`); the second infantry is asked only while the box holds two mechs (`supply::allowed(.., 2)`) | core `action_card_effects3::refit_troops_for_a_naaz_seat_places_the_eidolon_and_announces_each`; engine lockstep `action_dispatch_live_tests::refit_troops_for_a_naaz_seat_chosen_from_the_menu_places_the_eidolon_like_the_string` (events, `NAAZ_MECH_PLACED` staged, position, dice streams) |

`Unknown::UnportedCard` is raised only by Skilled Retreat at a Last Bastion table (a switched-off faction; `Disabled::LastBastion`); no lobby game reaches it. The cards Upgrade, Decoy, Harness Energy, Experimental Battlestation and Scramble are nested in the live hosts since lane H and no game of the sweeps stopped at one of them.

## 2. Edits of assertions (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` `a_native_game_plays_the_first_turns_and_stops_with_a_counted_gap_...` | replaced by `a_native_game_plays_a_whole_game_with_zero_gaps` (`End::Finished`, 9 rounds / 1,174 steps, 26 phases, fingerprint `2e93df9fa00fa982`, `g.step() == Step::Finished`) | the pinned gap (K1) is gone; the whole game plays. `--quick --twice`: the same fingerprint both times |
| `effects.rs` unit `bail_summit_declines_while_a_command_token_is_held_in_a_mahact_pool` | deleted (K2 above replaces it with the positive test) | the bail it pinned is gone |
| `effects.rs` unit `bail_courageous_to_the_end_declines_at_a_table_with_a_naaz_or_nekro_seat` | now `courageous_to_the_end_and_intercept_are_played_at_a_table_with_a_naaz_or_nekro_seat` (`is_some`) | K3 |
| `effects.rs` unit `bail_refit_troops_declines_without_a_plain_mech` | renamed `..._without_a_mech_to_place`; the assertions are the same (no unit in the facts: declined) | K4 changed what "no mech" means, not the bail |
| `ti4-engine` `action_cards_announce_tests::bail_intercept_at_a_table_with_a_nekro_seat_keeps_the_string_announcement` | now an agreement test, with an Alastor choice | K3: the dual compares the core window with the string at such a table (`(1, 0)` compared, zero differences) |
| `ti4-engine` `action_cards_b6d_tests::courageous_to_the_end_against_a_neutral_opponent_is_the_cores_and_a_naaz_seat_is_counted` | the Naaz table is `(1, 0)` (carried) and every case now also asserts ZERO differences (it asserted only the counts) | K3 |
| `live/plays.rs` `the_extra_facts_of_a_seat_name_its_forms_and_the_naaz_mech_is_declined` | `naaz.mech == (Some(Unit::Eidolon), 0)` | K4 |
| bridge `menu_sources::the_cards_and_partners_listed_reach_every_shape` | added `gaps == 0` and `jam_offered > 0 && jam_withheld > 0` | strengthened, not loosened |

No assertion was loosened.

## 3. The sweeps (clean binary: `native_play.sh`, gate profile, `te` card set)

`--quick` (sol, 4 players, seed 1): **finished, zero gaps**, 9 rounds, 1,174 steps (int. 14: 8 rounds, 967 steps, then `Menu(ActionCard)`).

`--factions all --players 3,4,5,6 --seeds 1` (96 games): **83 finish with zero gaps** (int. 14: 45; `Menu(ActionCard)`: 38 -> 0). First gap per game (FIN finished, MF `MissingFact`, ZU `Menu(Unrouted(5, Z'eu))`, CU `UnportedAftermath(CombatUnported)`, IE `Unported(IntegratedEconomy)`):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 817 FIN | 1174 FIN | 909 FIN | 1054 FIN |
| hacan | 1039 FIN | 1358 FIN | 1356 FIN | 1494 FIN |
| letnev | 765 FIN | 1079 FIN | 972 FIN | 1082 FIN |
| xxcha | 841 FIN | 959 FIN | 871 FIN | 1042 FIN |
| jolnar | 727 FIN | 1023 FIN | 527 CU | 1076 FIN |
| l1z1x | 702 FIN | 986 FIN | 1040 FIN | 1165 FIN |
| arborec | 807 FIN | 1050 FIN | 1050 FIN | 1054 FIN |
| mentak | 710 FIN | 1086 FIN | 988 FIN | 1040 FIN |
| muaat | 812 FIN | 702 IE | 1017 FIN | 1234 FIN |
| naalu | 154 ZU | 219 ZU | 192 MF | 161 ZU |
| nekro | 740 FIN | 1153 FIN | 1068 FIN | 1054 FIN |
| saar | 643 FIN | 1151 FIN | 1074 FIN | 1166 FIN |
| sardakk | 624 FIN | 1098 FIN | 522 CU | 1136 FIN |
| winnu | 642 FIN | 1176 FIN | 829 FIN | 1173 FIN |
| yin | 762 FIN | 965 FIN | 1029 FIN | 1090 FIN |
| yssaril | 746 FIN | 1095 FIN | 1082 FIN | 1216 FIN |
| ghost | 718 FIN | 1197 FIN | 920 FIN | 1027 FIN |
| argent | 651 FIN | 194 MF | 900 FIN | 1232 FIN |
| cabal | 758 FIN | 1096 FIN | 1029 FIN | 1042 FIN |
| empyrean | 118 MF | 1147 FIN | 174 MF | 1059 FIN |
| mahact | 653 FIN | 1178 FIN | 1070 FIN | 1291 FIN |
| naaz | 741 FIN | 1179 FIN | 150 MF | 195 MF |
| nomad | 645 FIN | 1240 FIN | 791 IE | 1068 FIN |
| titans | 722 FIN | 1047 FIN | 992 FIN | 1222 FIN |

Causes of the 13: ZU 3 (Naalu, Z'eu), MF 6 (tactical 3, aftermath 1, menu 2), CU 2, IE 2.

`--first-seed 2 --seeds 3` (288 games): **208 finish** (int. 14: 59; `Menu(ActionCard)`: 172 -> 0). The 80: MF 40 (tactical 26, aftermath 8, menu 5, phase 1), ZU 26, `PeaceAccords` 10 (menu 9, secondary 1: Freelancers + Peace Accords), CU 2, IE 1, `StrategyAbility` 1.

No game of any sweep stops at an action card (`PlayFacts`, `UnportedCard`, `CardCost`, `ImmediateCardPlay`, `MenuGap::ActionCard` all zero).

### Deeper look (exploration binary, NOT committed): Z'eu skipped by the bot, seeds 5-12 and 13-36 (768 + 2,304 games)

With a local harness hack (the bot never picks the Naalu agent's action, a `TI4_EXPLORE_NO_ZEU` env var in `Bot::ask`, plus a debug print of where each `Unknown` is raised via `#[track_caller]`) the games run past Z'eu: 677 of 768 and **1,642 of 2,304** finish; ~6,100 action cards were played in the 2,304 games. First gaps of the rest: `aftermath MissingFact` 201, `secondary TurnGap(StrategyAbility)` 176, `ask.action_menu MissingFact` 119, `tactical MissingFact` 95, CU 21, Z'eu (borrowed) 16, IE 6, others <= 6 each.
None is an action card. Where they come from (the raise sites, found with the track-caller print):
* `MissingFact`: `ti4-engine-core-factions/src/cb_economy/argent/timing.rs:60` (Argent flagship destinations; most of the `aftermath`/`menu` stops, 31 of the 768), `ce_movement/empyrean/timing.rs:180` (18), `rules/reactions/l1z1x_agent.rs:70` (13), `cc_ground/mentak/timing.rs:117`. Faction lanes.
* `StrategyAbility` in `secondary`: the follower chose the waiver `follow|waived|0|agent|57` (Jae Mir Kan, Mahact agent) with a Nomad at the table: `strategic/facts.rs::waivers_of` makes that waiver `WaiverEffect::Unported` ("heard by the Nomad's suite"). 176 of the 2,304 games.

The `--shadow` of 96 games (seed 5) stops at the known classes only (production order 42, strategic close order ~31, Mitosis 11, the Yssaril secondary 2); no difference is an action card.

## 4. Checks (cheap; private target `/root/deploytmp/core-k`, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, profile `lockstep` with debug assertions for the three core crates; deleted at the end)

| Check | Result |
|---|---|
| core tests: `action_dispatch` 68, `action_card_effects` 25, `..effects2` 27, `..effects3` 21, plus action_cards, action_cards_simple, action_cards_announce, action_card_rerolls, action_card_probe_limit, combat_machine, windows_misc, w11_neutral, hero_windows, kernel_fx, reactions, turn_jobs2, staged_cards, w16_judges | all passed |
| `ti4-engine-core --lib` | 475 passed (the two bail tests removed/renamed, one added: net as before) |
| `ti4-engine --lib` filters `action_card combat_live core_combat reactions_core live:: strategic_live turn_live` | 497 run; the 2 pins of the old bail (K3) became the agreement tests above; then `intercept courageous` 15 passed; `action_dispatch_live plays::` 48 passed |
| bridge `--test menu_sources --test play_facts` | 3 + 3 passed; `--lib native_play` 28 passed |
| `check_layers.py core`, `lint_core.sh` | 574 files 0 violations; 576 files 0 hits |
| `clippy_core.sh` | ok (rc 0, engine warnings 41, baseline 41) |
| mutation (single files, union of the named test binaries; `D4_COVERS_FILE=/dev/null`) | see section 5 |

Not run (user rule): golden / wide dual, benches, soak, census, workspace clippy beyond `clippy_core.sh`, the ratchet, `end_of_d1.sh`, Playwright.

## 5. Mutation checks of the new core halves

| File / functions | Tests | Result |
|---|---|---|
| `driver/menu_cards.rs` `action_cards` | core `action_dispatch` | 22 points: the Signal Jamming lines (199-205) all killed after the two-seat case (`> 1` -> `> 2` survived once); the 13 survivors are the older lines 188-197, 207-222 (the bill and the bars), killed by the bridge `menu_sources` and the engine lockstep in the earlier KM batches |
| `rules/action_cards/effects.rs` `jam_systems, mechs_left, refit, choose, before, begin_courageous` | core `action_card_effects{,2,3}` | 121 points: 90 killed; survivors on new lines: `mechs_left` and `jam_systems` position defaults (unreachable, `// mutants:skip` with reason), `jam_systems` conditions (killed by `action_dispatch`: 6/6); the rest are older lines of `before` / `begin_courageous` |
| `effects.rs` `apply` (Refit arm), `jam_systems`, `refit`, `mechs_left` | `action_dispatch` + `action_card_effects{,2,3}` | 207 points: 145 killed; on new lines one survivor, `return false` of the Naaz branch: EQUIVALENT (the box is empty after the placement, so the next options are empty and the card ends), `// mutants:skip` with the reason |
| `kernel/read.rs` (the moved `ships_of` family) | combat tests + `action_card_effects{,2}` | 25 points: first run 17 killed; the direct test `ships_of_adds_the_units_that_fight_from_a_planet` (Maximum with and without a ship, a rival's ship, the Alastor's choice on one of two planets, owner and index) -> 24 killed, 1 survivor: `Rd::new` `blanked: false` (older line, not changed) |
| `live/plays.rs`, `reactions_core.rs` (engine, one conditional each) | unit test of both branches; bridge `play_facts` compares string and core facts | not mutation-run (the engine lib test build is not a single-file check) |

## 6. What remains (action-card area and its neighbours)

* **Display facts of card picks in the native render** (UI only, no bail): the core's rendering of the pick questions of Refit Troops, Scuttle, Upgrade, Lucky Shot (`details.units`: system, planet, unit, owner, damaged, cost, and the per-option `system`) and Repeal Law (`details.laws`) has none of the string's `details`; the dual path copies them from the recorded string question (`action_cards_view::recorded_details`), `live/details.rs` does not. A card played from the menu or a window in a native game asks these questions without the details. The Naaz Refit lockstep keeps a single infantry so no pick is asked; a lockstep of the pick questions needs `live/details.rs` to build them (units from the option payload and the board, `agenda_details` for the laws).
* No lockstep of Signal Jamming's EFFECT through the menu with a map (the harness of `action_dispatch_live_tests` loads no Map slice; the offer is compared by `menu_sources`, the effect by `action_cards_b6d_tests::signal_jamming_closes_a_system_near_the_holders_ships_in_the_core_window`).
* The next first gaps are not action cards (section 3): Z'eu (every Naalu game, and 16 of 2,304 elsewhere), the faction `MissingFact`s (Argent, Empyrean, L1Z1X agent, Mentak), Jae Mir Kan's waiver with a Nomad, `CombatUnported`, Integrated Economy in the invasion, Freelancers + Peace Accords.

## 7. Reproduce

```
export CARGO_TARGET_DIR=/root/deploytmp/core-k CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
tools/engine_core/native_play.sh --factions all --players 3,4,5,6 --seeds 1
tools/engine_core/native_play.sh --factions all --players 3,4,5,6 --seeds 3 --first-seed 2
```
