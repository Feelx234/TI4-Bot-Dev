# D2 lane F2e (`core-d2-c3-dispatch4`): the rest of the menu options routed, resume note (2026-11-01)

Branch `core-d2-c3-dispatch4`, cut from `core-d2-c3-dispatch3` (`8b212698a`). Never pushed; `nightly-fixes-2026-10-26` untouched. Only the LATEST rules version is supported.
Read `docs/KM4_RESUME.md` (F2d), `docs/KM3_RESUME.md` (F2c), `docs/KM2_RESUME.md` (the dispatch) first. Pure refactor: RULES_VERSION, serde, fixtures and fingerprints are unchanged.
Based on `core-d2-c3-dispatch3` only: M6c's `resume_strategy` was NOT needed (see Mathis Mathinus below), so `core-m6c-tactical` is not merged.

## 1. Routed now (`Opt` -> core body), additions to KM4 section 1

| kind | option | file | evidence |
|---|---|---|---|
| leader (agent, ACTION) | Umbat (Muaat), also through Ssruu | `turn/leader.rs`, `turn/produce.rs` (new) | core `umbat_*`; lockstep `umbat_chosen_..`, `umbat_for_another_player_..` |
| leader (hero, ACTION) | Letani Miasmiala (Arborec) | same | core `letani_miasmiala_..`, Auto-Factories and Prophecy tests; lockstep |
| leader (hero) | Rin (Jol-Nar): corpus order from `codes.agenda.tech_corpus`, alias order from `plays.tech_order` | `leader.rs` | core `rin_*`; lockstep `rin_chosen_..` |
| leader (hero) | Riftwalker Meian (Ghost): `SwapFacts` from the core galaxy (wormholes, the Creuss/Nexus/Fracture exclusions) | `leader.rs` | core only (the lockstep harness loads no map); shadow |
| leader (hero) | UNIT.DSGN.FLAYESH, Kyver, It Feeds on Carrion, Hesh and Prit, Mirik Aun Sissiri, Conservator Procyon | `turn/hero.rs` (new) | core tests each; lockstep for Flayesh, Kyver, Hesh and Prit (Carrion, Mirik, Procyon need a map: core + shadow) |
| leader (hero) | Mathis Mathinus (Winnu): the primary performed, then `LeaderStrategy` + `select_leader_strategy_followers` (strategic lane E) | `hero.rs` | core `mathis_..`; lockstep with the Trade primary |
| relic | Enigmatic Device, Maw of Worlds, Stellar Converter, Titan Prototype, The Silver Flame (a die on the dice stream, purpose `silver_flame`) | `turn/relic.rs` | core tests each; lockstep for all but the Converter (map) |
| exploration card | Enigmatic Device cards (`Opt::Device`): own question variant `RAsk::DeviceTechnology { card }` ("research which technology with the Enigmatic Device") | `relic.rs::use_device`, `choice/relic.rs`, `render/relic.rs` | core + lockstep |
| faction | Wormhole Generator (Ghost) | `turn/faction.rs` | core (map) |
| faction (menu gaps closed) | Nekro ???_ERROR_ERROR_??? (place PDS, repair, draw), Titans Terraform (one option per controlled non-home planet), Cabal Vortex: now LISTED by `menu_faction::faction` and routed. `MenuGap::Faction` is no longer raised | `menu_faction.rs`, `faction.rs` | core + lockstep (C4R x3, Terraform); Vortex core only (map) |

`production by an ability` (Umbat, Miasmiala): facts from `rules::production::probe::ability_facts` (limit 2 and cost ceiling 4 for Umbat, 1000 for Miasmiala); the string adapter's tail after the child (`leaders_core::announce_uses`) is `turn/produce.rs::finish_uses`: planet-value swap lapses, Hacan's Auto-Factories pays a fleet token for three non-fighter ships, the Prophecy of Ixth is discarded unless two fighters were made, `UNITS_PRODUCED` (source `ability`) announced at once.

Other changes: `TurnCodes` gets `silver_flame` and `nekrohero` (interned in `ti4-engine/src/live/turn.rs`); `driver/agenda/ground.rs` announces the ground forces of UNIT.DSGN.FLAYESH (cause `nekrohero`, before the goods: found by the lockstep); `ROUTED_FACTION` in `native_subject.rs` and the `play_area:` lend are updated; small visibility changes (`end_value_swap`, `announce_made`, `place_tokens`, `Umbat::candidates`, `PlaceUnits::has_spot`, `Vortex::offered`).

## 2. NOT routed (counted gap `Unrouted(5, key)`), with owners

* Airo Shir Aur (Mahact hero) and Dannel of the Tenth (Yin hero): both run a combat / invasion as a child; their windows take `Prepared`/`YinFacts` (the machine environment and fields of a fresh fight). Owner: M5/M6 (combat and invasion preparation), then this table.
* Mathis Mathinus with Thunder's Edge Warfare: the free tactical action (`TurnGap::FreeTactical`, M6c `open_free`/`resume_strategy`).
* Z'eu: lane H4 (dropped content here).

## 3. `native_play --shadow` after the lane

The table of KM4 section 2 is unchanged game for game (same steps, same stops; every stop is another lane's): the games of this lane's routes were not in it. The wider sweep (`--factions all --players 3,4,5,6 --seeds 3 --first-seed 20 --shadow`, 288 games, 0 clean finishes): first gaps `UnportedTactical(ActivationHook)` 207, `TurnGap(StrategyAbility)` 64, `PeaceAccords` 2, and **`Unrouted` 0** (KM4: 5 Umbat + Terraform). First differences (16 games): strategic-action close order (`STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT`, owner E) 8, space cannon head / Saar and Nomad agent reactions (G/H1) 4, Cabal/other reaction questions, the rest single games. Owners: ActivationHook = M6; StrategyAbility = M6c/M6d (Thunder's Edge Warfare, Doctor Sucaban); PeaceAccords = the Xxcha annexation window (lane E/H).

## 4. Mutation debt (single files, `tools/engine_core/mutants.sh`, Haiku-free background scripts on committed snapshots in scratch worktrees; union of the core test `action_dispatch` and the engine lockstep `action_dispatch_live_tests`, `D4_COVERS_FILE=/dev/null`; a survivor counts only when BOTH runs miss it)

F2d debt, re-run at `8b212698a` (before this lane's code): `leader.rs` 29/29 killed by the core tests alone; `relic.rs` 4/4 and `technology.rs` 3/3 killed in the union; `faction.rs` one survivor in the union (#34, `matches!(mode, FabMode::Relic { bmf: true })`: the question's source `bmf` vs `fabrication`), killed by the new `black_market_forgery_and_fabrication_ask_under_their_own_source` (the test table interns nothing, so it sets two distinct codes). The survivors that were only in one of the two runs (the `== Flow::Halt` arms of Orbital Drop, Stall Tactics, Mageon: core only; the lockstep ones: every `roster`/box default) are killed by the other run.

Final numbers at `12f4c89b6` (this lane's code included), mutants run / killed by the core test alone / union survivors:

| file | mutants | core `action_dispatch` | union survivors |
|---|---|---|---|
| `turn/leader.rs` | 45 | 45 | 0 |
| `turn/hero.rs` (new) | 45 | 45 | 0 |
| `turn/produce.rs` (new) | 15 | 15 | 0 |
| `turn/technology.rs` | 3 | 3 | 0 |
| `turn/relic.rs` | 41 | 39 | 0 (the other 2 are killed by the lockstep) |
| `turn/faction.rs` | 45 | 36 | 0 (the 9 `Halt` arms are killed by the lockstep) |
| `rules/explore_windows.rs` (coordinator's list, `FN=mech_forbidden,gain_goods,rewarded,place,step`, core tests `exploring explore_windows exploring_freelancers`) | 85 | 85 | 0 |
| `driver/windows/invasion/fight.rs` (`advance_control explore_first_claim finish_control_gain`, core `invasion_composer invasion_yin invasion_windows invasion_extra`, lockstep `invasion_live`) | 21 | 18 | 0 (3 killed by the lockstep) |

The coordinator's late report (old commit `7e3a196e5`): on this branch only two of the listed points survived the core tests (`PayMechOrInfantry::step` `||` -> `&&` at l.404 and the merchant replenish check at l.1246), killed now by `an_infantry_on_a_planet_nobody_controls_still_asks` and the added `unsupported().is_none()` assertion of `a_merchant_station_a_mercenary_outfit_and_a_mech_or_infantry_that_pays`. For `fight.rs` the `1 -> 2` points at l.505 and l.529 are killed by `a_planet_the_invader_lost_the_fight_for_is_passed_over_and_the_next_one_is_taken` and `two_planets_conquered_in_one_invasion_are_both_taken`, l.521 by `losing_a_home_planet_does_not_end_the_chain_of_control`; l.546 (`report.explored`, read only by the string dual adapter's report) and l.565 (`rules_at_least(1)`, true at the one version played) are marked `// mutants:skip` with those reasons.

Marked `// mutants:skip` in this lane (reason on the line): the failed-verdict twins of the relic uses and of the Device card (the menu offers them only when they resolve), the `Code(0)` source and `placed > 0` of the capped PDS placement, the `home` flag of Terraform (the menu lists no home planet), the Umbat ceiling (4 and 5 offer the same units: no unit costs 5, halves are 1 2 4 6 8 16 20 24), `is_colour` (a specialty is always a colour), the `effects` flag of `SeatReach` and the token test of Mirik (the window narrows `enterable` to token systems), the off-map wormhole test, the Imperial/score announcements of Mathis (the same code as the strategic arm's `announce`), and the `Halt` composites ("a halted step is rolled back by the driver whatever the job does after").

## 5. Checks run

`check_layers.py core` 516 files and `lint_core.sh` 518 files, 0 violations/hits. Core tests (20 named binaries, all pass): `turn_jobs turn_jobs2 turn_end_job aftermath_chain leader_windows leader_windows_agents leader_heroes leaders leadership strategic_arms action_dispatch faction_components hero_windows leader_production promissory_windows relic_windows invasion_composer invasion_yin explore_windows exploring`; `action_dispatch` has 66 tests (KM4: 30). `ti4-engine-core --lib` 477 passed. `ti4-engine --lib` (whole): 5166 passed, 0 failed, 7 ignored; `action_dispatch_live_tests` 38 (KM4: 21). Bridge lib 105 passed. `clippy_core.sh` fails on lines this lane did not write: `ti4-engine-core-tests/tests/combat_fight.rs:62` (`show_neutral` specified twice, an error from the `core-d2-c1-strategic` merge `361fad59c`), warnings in `status_phase.rs`, `turn_jobs2.rs` (`without_map` unused), `native_subject.rs:1045`, and three engine warnings not in the baseline (`live/trade.rs`, `trade_live_tests.rs`, `turn_live_tests.rs` `native_stop`); none in a file or line of this lane. Not run (standing rule): golden/wide dual, benches, soak, census, workspace clippy, the ratchet.

## 6. Next (in order)

1. Airo Shir Aur and Dannel of the Tenth (the combat/invasion preparation, M5/M6), then Mathis with Thunder's Edge Warfare (M6c `open_free`).
2. The other lanes' stops, by volume in the sweep: `UnportedTactical(ActivationHook)` (207 of 288), `StrategyAbility` (64), the strategic action's close order against a spent token, the space cannon at the head of the aftermath, the Saar/Nomad agent reactions, Mitosis in the status phase, movement facts on hyperlane maps.
3. Lockstep with a map for the map-bound routes (Meian, Carrion, Mirik, Procyon, Stellar Converter, Wormhole Generator, Vortex): they are covered by core tests and the shadow only.
