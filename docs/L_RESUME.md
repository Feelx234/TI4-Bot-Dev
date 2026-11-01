# Lane L (`core-l-leftovers`): the leftovers of the native playthrough, resume note (2026-11-01)

Branch `core-l-leftovers`, cut from `core-integration-14` @ `4766eafcc`. Never pushed; `nightly-fixes-2026-10-26` untouched. Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (the one new
state variant, `EThen::Enforced`, is core-only payload of a hosted exploration, never stored; `OneLink` and `TurnGap` are core-internal).
Read `docs/INTEGRATION14.md` (sections 5, 6, 8) first. (`docs/BAIL_MAP.md` named in the brief does not exist in this tree; the bail list is section 6 / 8 of the integration note.)

User directive behind it: a whole game must play natively with zero bails on the agreed content (only Thunder's Edge content beyond the TE strategy-card set, and dropped features, may stay out). Lane K does the
action card from the menu / secondary (`Menu(ActionCard)`), lane J the shadow differences (close order, hyperlane, production order): nothing of theirs was touched.

## 1. What was done, in the order of the brief

| # | Stop (96-game sweep, `--factions all --players 3,4,5,6 --seeds 1`) | Cause | Fix | Where |
|---|---|---|---|---|
| 1 | `Menu(Unrouted(5, Z'eu))` in 3 games (every Naalu game, step 154 to 219) | the Naalu agent had no body | `ZeuAction` window (asks the system, the decline alone when there is none, as `Game::perform_zeu` does); `zeu()` of the leader route: exhaust, Extreme Duress, `COMPONENT_ACTION_RESOLVED`, then `tactical::begin_free` (no token, `SYSTEM_ACTIVATED`, `FREE_TACTICAL_ACTION`); a declined question is `COMPONENT_ACTION_FAILED` and the key is withheld. The question is the string's byte for byte (`zeu_activate_system`, kind `activate`, label `activate {system} without a command token`, optional). **Only the seat itself is offered** (the string's `perform_zeu` does not offer another player either; the brief's "chosen seat" is not a rule of the string) | `rules/leader_windows.rs` (`ZeuAction`), `choice/leaders.rs` (`ZeuSystem`), `ti4-engine/src/render/leaders.rs`, `driver/turn/leader.rs` (`zeu`, `routed`), `rules/tactical/mod.rs` (`holds_leave` exported) |
| 2a | `MissingFact`, Naaz/Naalu tables (naaz 5p/6p, naalu 5p/6p; tactical 3 + menu 1) | the L1Z1X agent (`l1z1x_agent`) asked for THE faction mech of the target among every source: Naaz has 3 forms in PoK and Naalu 3 (Iconoclast, Omega, TE), so `faction_mech` was ambiguous | the form is the roster's (`placed_unit`, the table's `facts.roster`) among the sources in play; without a roster (the lockstep windows) the scope of the kernel filters, ambiguity stays unknown. A Naaz mech put on the board stages `NAAZ_MECH_PLACED`, as `supply::stage_naaz_mech_placed` does (the old code answered "the string's" with `MissingFact` for every Naaz target) | `rules/reactions/l1z1x_agent.rs`, `cd_registry/yssaril/ssruu.rs` (the Ssruu copy, same signature) |
| 2b | `MissingFact`, Argent (argent 4p, aftermath) | `destinations()` read the neighbours the string adapter records (`facts.reach`); a live game records none | `query::reach::neighbours_of` (the seat's movement adjacency from the position's map); the recorded neighbours still win; no map slice loaded -> unknown | `ti4-engine-core-factions/src/cb_economy/argent/timing.rs`, `query/reach.rs` |
| 2c | `MissingFact`, Empyrean (empyrean 3p/5p, menu) | `near()` of Dynamo and the Watcher read the pair the string recorded (`facts.adjacent`) | the recorded pair wins, else the position's own galaxy | `ce_movement/empyrean/timing.rs` |
| 2d | `CombatUnported` (jolnar 5p, sardakk 5p, aftermath) | the machine stood at a roller whose form its texts did not name: a Letnev defender played Reveal Prototype at the start of round 1 ("spend 4 resources to research a unit upgrade"), its Destroyer I became a Destroyer II and `every_roller_is_named` failed | the texts of the fight (`combat_live::combat_texts`) name the upgrade a seat owns but does not field and, for a seat that can research while the combat goes on (holds Reveal Prototype, or is the Nekro), every other form of its base types | `ti4-engine/src/combat_live.rs` |
| 2e | `Unported(IntegratedEconomy)` inside the invasion (muaat 4p, nomad 5p) | `finish_control_gain` refused the invader's Integrated Economy | the production on the planet gained, asked on the tape through `strategic::perform::integrated_economy` (the body of Peace Accords too), between the capture's announcements and the exploration, as `technology::control_gained` is called | `driver/windows/invasion/fight.rs`, `driver/windows/strategic/perform.rs` |
| 3a | `Unrouted(5, Airo Shir Aur)` | the Mahact hero runs a space combat as a child and needs the machine environment | `mahact_pairs` (pairs in board order), `mahact_prepared` (a clone of the position with the fleet moved, the barring mark, the sides written; `env_of`, the limits, the fields of a fresh window), `airo_shir_aur`; the engine's combat texts name the types of the pairs before the card moves the fleet (`hero_fight_types`) | `driver/turn/hero.rs`, `ti4-engine/src/combat_live.rs` |
| 3b | `Unrouted(5, Dannel of the Tenth)` | the Yin hero runs the committed ground fights | `FightFacts` built from the invasion codes (`dannel`), `yin_spots` (planets outside the home systems, in map order); the Thunder's Edge corners the commitment window does not carry (a neutral side, Proxima Targeting VI, Alarum, L1Z1X's Fealty Uplink, a coexisting planet) are the counted gap `TurnGap::HeroFight` | `driver/turn/hero.rs`, `kernel/turn_codes.rs` |
| 3c | `TurnGap::FreeTactical` (Mathis Mathinus with the TE Warfare primary) | the free action of a hero's primary | `tactical::begin_free` inside the hero's arm (the activation is announced where the string announces it), the hero stays in play (`LeaderStrategy`) and the followers are selected by `strategic::after` at the end of the action | `driver/turn/hero.rs` |
| 4a | `EWhy::Freelancers` (a unit over the capacity of its system) | the exploration's `ProduceOne` ended `unsupported` | the enforcement of the system is a `Spawn::Seeing` child of the card (`EThen::Enforced`, `OneLink::limits`); the card keeps its verdict | `rules/explore_windows.rs`, `state/explore_run.rs`, `kernel/children.rs`, `driver/children.rs` |
| 4b | the research window bail for more than two waiver offers | only the Cabal (Riftmeld) and the Yin (Brother Omar) offer a research waiver, one each: a third offer is not a position of any game | the guard stays, documented and `mutants:skip` (a question of two places is exact) | `rules/production/strategy_cards.rs` |
| 4c | the Yin hero dual adapter's declines (neutral side, Proxima Targeting VI, Alarum, L1Z1X Fealty Uplink, coexisting planet) | all five are Thunder's Edge content beyond the strategy-card set | named gap `TurnGap::HeroFight` in the native route (see 3b); the dual adapter keeps its declines | `driver/turn/hero.rs` |
| 5 | the third copy of `announce_staged_cards` | `driver/windows/status_finish.rs` | a thin wrapper (Result -> `Flow`) of `kernel::fx_cards::announce_staged_cards`, the one body; the calls after faction/leader component actions (`component::announce_staged`) and in the agenda reveal and close (`rules/agenda/{reveal,close}.rs`) were native already | `driver/windows/status_finish.rs` |

Found by the lockstep and fixed (not in the brief): the combat texts and the new route read the galaxy of a lend that loaded no map slice (`CoreState::galaxy` debug-asserts it): every `action_dispatch_live_tests` test failed (39).
Guards: `loaded.has(Slice::Map)` in `hero_fight_types`, `neighbours_of`, Empyrean's `near` and the Z'eu route (no map: the string without a galaxy also offers the decline alone).

## 2. The gap matrix (`--factions all --players 3,4,5,6 --seeds 1`, 96 games)

Integration 14 started at **45 games finished** (AC 38, MF 6, ZU 3, CU 2, IE 2). Now **51 finish with zero gaps**, and **every one of the other 45 stops at the same first gap: `TurnGap(Menu(ActionCard))`** (lane K).
No `MissingFact`, `Unrouted`, `CombatUnported` or `IntegratedEconomy` first gap is left. First gap per game: steps, then FIN (finished) or AC (`Menu(ActionCard)`).

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 817 FIN | 967 AC | 777 AC | 819 AC |
| hacan | 1039 FIN | 1358 FIN | 1356 FIN | 1164 AC |
| letnev | 765 FIN | 989 AC | 927 AC | 857 AC |
| xxcha | 841 FIN | 890 AC | 871 FIN | 782 AC |
| jolnar | 727 FIN | 885 AC | 845 AC | 1076 FIN |
| l1z1x | 702 FIN | 986 FIN | 928 AC | 908 AC |
| arborec | 807 FIN | 1050 FIN | 944 AC | 856 AC |
| mentak | 710 FIN | 1003 AC | 988 FIN | 752 AC |
| muaat | 812 FIN | 1190 FIN | 881 AC | 985 AC |
| naalu | 813 FIN | 915 AC | 977 FIN | 799 AC |
| nekro | 740 FIN | 1153 FIN | 921 AC | 822 AC |
| saar | 643 FIN | 1151 FIN | 1074 FIN | 793 AC |
| sardakk | 624 FIN | 1098 FIN | 726 AC | 762 AC |
| winnu | 642 FIN | 1176 FIN | 829 FIN | 920 AC |
| yin | 762 FIN | 758 AC | 782 AC | 998 AC |
| yssaril | 746 FIN | 848 AC | 908 AC | 1216 FIN |
| ghost | 718 FIN | 1197 FIN | 792 AC | 1027 FIN |
| argent | 651 FIN | 1021 AC | 658 AC | 970 AC |
| cabal | 758 FIN | 1096 FIN | 896 AC | 842 AC |
| empyrean | 631 FIN | 1147 FIN | 914 FIN | 783 AC |
| mahact | 653 FIN | 1178 FIN | 1070 FIN | 897 AC |
| naaz | 741 FIN | 1179 FIN | 899 AC | 1159 AC |
| nomad | 645 FIN | 1240 FIN | 959 FIN | 829 AC |
| titans | 722 FIN | 924 AC | 992 FIN | 1088 AC |

By island: B turn flow 36 gaps (`ask.action_menu`), E strategic action 9 (`secondary`), all `Menu(ActionCard)`. Native steps by arm: turn prelude 14,993; action menu 15,475; tactical 17,348; secondary 10,037; aftermath 6,143; tokens 6,691; draft 5,218;
closing menu 4,124; phase 2,361; trade 1,965; scoring 1,034; event scoring 840; status 789; strategy announce 96; starting tech 12.

## 3. Tests (each fails on the code before the fix)

R = seen to fail before / by construction (the old code answered `Unrouted`, bailed, or asked nothing).

| Test | File | What it guards |
|---|---|---|
| `zeu_chosen_from_the_menu_begins_a_free_tactical_action_without_a_token_in_a_system_without_the_seats_token`, `zeu_offers_no_home_or_tokened_system_and_a_decline_fails_the_action` | `ti4-engine-core-tests/tests/action_dispatch.rs` | the question's options (no home, no own token, the decline), exhaust, Extreme Duress, order of `COMPONENT_ACTION_RESOLVED` before `FREE_TACTICAL_ACTION`, one `SYSTEM_ACTIVATED`, no token spent; the decline fails and is withheld (R: `Unrouted`) |
| `zeu_chosen_from_the_menu_asks_the_system_and_a_decline_fails_the_action_like_the_string` | `ti4-engine/src/action_dispatch_live_tests.rs` | lockstep against the string (plain and `TI4_AREAS=all=dual`); the harness loads no map, so the question stands with the decline alone on both sides |
| `airo_shir_aur_chosen_from_the_menu_moves_the_fleet_fights_and_is_purged` (replaces the example of "an option no window runs") | `action_dispatch.rs` | the pair, the move, the fight, the purge, the active system back (R: `Unrouted`) |
| `dannel_of_the_tenth_chosen_from_the_menu_commits_infantry_to_an_empty_planet_and_takes_it` | `action_dispatch.rs` | the commit questions with the decline, the landing, the control (R: `Unrouted`) |
| `mathis_mathinus_with_the_thunders_edge_warfare_primary_begins_the_free_tactical_action_and_waits_for_its_end` | `action_dispatch.rs` | the free action is open at its movement, the hero stays in play, no follower selected (R: `TurnGap::FreeTactical`) |
| `a_planet_taken_with_integrated_economy_asks_the_production_and_the_invasion_goes_on` | `ti4-engine-core-tests/tests/invasion_composer.rs` | the Economy question inside the invasion (R: `Unported(IntegratedEconomy)`) |
| `a_conquest_with_integrated_economy_asks_the_production_natively_as_the_string_window_does` | `ti4-engine/src/invasion_live.rs` | lockstep against the string window, six invaders (R: the native run stopped) |
| `a_unit_over_the_capacity_of_its_system_is_followed_by_the_enforcement_of_the_system` | `exploring_freelancers.rs` | Freelancers' two fighters over a lone cruiser: no bail, the fleet back at capacity, the card resolved (R: `EWhy::Freelancers`) |
| `a_naaz_target_gets_its_mech_and_the_placement_is_announced`, `an_exhausted_agent_is_not_offered` | `rules/reactions/l1z1x_agent.rs` | the Naaz target swaps and stages `NAAZ_MECH_PLACED` once (R: `MissingFact`) |
| `a_position_with_a_map_answers_the_pair_nobody_recorded` | `ce_movement/empyrean/timing.rs` | the position's map answers, the recorded pair wins (R: `Err(())`, unknown) |
| `the_destinations_follow_the_recorded_neighbours_else_the_position_own_map` | `cb_economy/argent/timing.rs` | recorded neighbours win, else the map, else unknown (R: `None`) |
| `the_neighbours_of_a_system_are_the_systems_next_to_it_on_the_map_in_system_order` | `tests/reach.rs` | `neighbours_of` |
| `the_texts_of_a_fight_name_the_upgrade_a_seat_owns_but_does_not_field_yet`, `the_texts_of_a_fight_name_the_forms_a_seat_holding_reveal_prototype_can_research`, `the_texts_of_the_step_that_plays_airo_shir_aur_name_the_types_of_the_pairs` | `ti4-engine/src/combat_live.rs` | the combat texts (R: the machine's `CombatUnported`) |

## 4. Mutation numbers

Single-file checks (`tools/engine_core/mutants.sh`, committed snapshot in the scratch worktree `/root/deploytmp/core-l-mutwt`, `D4_COVERS_FILE=/dev/null`, kinds `body,arith,cmp,logic,cond,lit,assign`, the D4 crate's named binaries
or the core lib as the test run). The numbers are the CORE tests alone (an upper bound on the survivors: the engine lockstep tests were not run per mutant). First round, then after the tests and the skips of this lane:

| file (what) | mutants run | first round: killed / survivors | final: killed / survivors |
|---|---|---|---|
| `rules/leader_windows.rs`, lines 690-769 (`ZeuAction`) | 10 (+2 not compilable) | 9 / 1 (`space_battle` of the context) | 10 / 0 |
| `driver/turn/leader.rs` (`zeu`) | 8 | 7 / 2 (the Mahact leave flag and the token filter: both dead, `activatable` already leaves the seat's own tokens out; the code was removed, the home filter is pinned by a seat-1 home on the map) | 8 / 0 |
| `driver/turn/hero.rs`, lines 267-500 (Mathis free action, Airo, Dannel) | 49 (+3) | 26 / 23 | 33 / 0 (4 more lines carry `mutants:skip` with a reason: Proxima Targeting VI and Alarum are dropped factions, the Dacxive facts need dice that win, `numbered` and `can_pay` are dead under the latest rules version / the payment environment, the early verdict of an empty box is not reachable from the menu) |
| `driver/windows/status_finish.rs` (`announce_staged_cards`) | 1 | 1 / 0 | 1 / 0 |
| `driver/windows/invasion/fight.rs` (`finish_control_gain`) | 8 | 5 / 3 | 6 / 2: both are `announce: true` of `ControlFacts` and `ExploreRun::new(.., true)`, which KM5 section 4 already lists as killed by the engine lockstep `invasion_live` only; the Integrated Economy branch has no survivor (the one it had, `== Halt` -> `!=`, is killed by the first-claim test) |
| `rules/explore_windows.rs` (`leave_producing`) | 3 | 2 / 1 | 3 / 0 |
| `rules/reactions/l1z1x_agent.rs` (whole file) | 22 | 16 / 6 | 22 / 0 |
| `query/reach.rs` (`neighbours_of`) | 4 | 3 / 1 (the `effects` flag: skipped with a reason) | 3 / 0 |
| `ce_movement/empyrean/timing.rs` (`near`, `any_near`) | 10 | 10 / 0 | not re-run |
| `cb_economy/argent/timing.rs` (`destinations`) | 3 | 3 / 0 | not re-run |

The engine side (`combat_live.rs`: `hero_fight_types`, the upgrade names of `combat_texts_in`, `researches_in_combat`) is the adapter half; its tests are the three engine tests of section 3 (not mutated: the engine lib is the heavy build).


## 5. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/core-l` (profile `lockstep`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

| Check | Result |
|---|---|
| `cargo test -p ti4-engine --lib` plain (final tip) | **5284 passed**, 0 failed, 7 ignored (integration 14: 5279; +5: the Z'eu lockstep, the Integrated Economy lockstep, three combat-text tests) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5284 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib`, `-factions --lib`, `-bridge --lib` plus the bridge's `kernel_env`, `play_facts`, `menu_sources` | 478, 151, 108 + (3, 3, 2) passed |
| the D4 crate (`ti4-engine-core-tests`, every binary, lockstep profile with debug assertions) | 154 binaries: **1935 passed, 0 failed** (integration 14: 153 / 1925) |
| `check_layers.py core`, `lint_core.sh` | 574 files, 0 violations; 576 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (core crates 0 warnings; engine 41 = baseline); the first run found one new engine warning of this lane (`iter().any` for `contains`), fixed |
| `native_play.sh --factions all --players 3,4,5,6 --seeds 1` (gate profile, `te` card set) | 96 games, 87,126 native steps: **51 finish with zero gaps**, the other 45 stop at `Menu(ActionCard)` (section 2). Run at the end on the final tip |
| the usage: five-hour window 1 % at the start, 7 % at the end | -- |

Things the checks found and the lane fixed on the way (besides the sweep's own causes): the map-slice assertion (39 lockstep tests, section 1), a test of mine that did not exercise what it claimed (Freelancers over the capacity: the space dock supports the fighters; the
test now puts three ships against a pool of two), the clippy warning. The lockstep harness does not list Z'eu for a mapless board with every system a home; the test puts a rival's ship on Mecatol.


Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond `clippy_core.sh`), `decision_replay`,
`TI4_CORE_STRICT=1`, Playwright. The long runs were background scripts of this session (no helper agents), as in integration 14.

## 6. What remains (for the zero-bail playthrough)

1. **`Menu(ActionCard)`: 45 of 96 games** (lane K): the action card played from the action menu (36) or from a secondary (9). It is the first gap of EVERY game that does not finish.
2. Shadow differences (lane J): the close order of a strategic action (`STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT`) stops `--shadow` after 17 to 51 steps of the Naalu games, so Z'eu / Airo / Dannel / Mathis-with-Warfare are covered by core tests (and Z'eu's decline by the string
   lockstep) and by the games finishing natively, not yet by a long shadow. Airo, Dannel and Mathis-with-Warfare have no string lockstep (the harness loads no map); they need the shadow once lane J's differences are gone.
3. Thunder's Edge content the native Dannel names as `TurnGap::HeroFight` (neutral side, Proxima Targeting VI, Alarum, Fealty Uplink, a coexisting planet); the dual adapter's declines for the same corners stay.
4. The other `Unknown::MissingFact` sites of the factions crate that no game of the sweep reaches (they read adapter facts a live game does not record: Dynamo's payment kernel is lent, the Yin / Mentak / Cabal / Sardakk / Mahact timing hooks that call `c.unknown(MissingFact)`) were not touched; none is the first gap of any of the 96 games.
5. The sweeps of other seeds (`--seeds 3 --first-seed 2`) were not re-run.
6. The heavy gates of integration 14 section 8 (golden and wide dual, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, benches, census, soak, ratchet, workspace clippy, `TI4_CORE_STRICT=1`) still wait for a zero-bail playthrough.

## 7. Tip

`git log -1 core-l-leftovers` (the last commit is this document's). The private target dir and the scratch worktree for the mutation runs were deleted.
