# Lane Q (`core-q-research`): the last native stops of integration 15, mutation of its new lines -- resume note (2026-11-01)

Branch `core-q-research`, cut from `core-integration-15` (`3ab5e6e64`). Never pushed; the live branch `nightly-fixes-2026-10-26` is untouched. Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints of the string engine unchanged (two small core/engine changes, below; no stored type touched).

## 1. The stops, with their causes (both found by single-game runs, `TI4_NATIVE_PLAY_TRACE=1` plus a temporary print)

INTEGRATION15 section 8 item 2 blamed the join of Doctor Sucaban x E2 x G5 for `TurnGap::StrategyAbility` in a Technology research (`Research::after_step`, `PayStep::took == None`). **That was not the cause.** In sol 6p seed 1 the print at the stop said
`sucaban=0 offer_at=0 who=Seat(1) cost=4 owed=4 paid=0 avail=5 opts=[] wm=5 planets=[] tg=0`: no Sucaban offer was ever made.

| # | games | cause | right | fix | regression tests (fail on the pre-fix code) |
|---|---|---|---|---|---|
| 1 | sol 6p s1, ghost 5p s1 (96-game set), l1z1x 4p s3, naalu 3p s4 (`--first-seed 2`) | The research gate of the core window (`Research::after_offers`, and `afford_inheritance` of `KernelHooks`) used `production::available`, which counts War Machine's five faces (the mark of the activation stays on the seat until the next activation) and the Triad. The string gates with `payment::affordable` (planets and goods only). A seat with nothing to spend but a stale War Machine mark was "able to pay" in the core, the pay step found no option and the window ended `failed` (a counted `StrategyAbility` gap); the string asks nothing and ends the research | string | `rules::production::payable` (= `available` minus the Triad and, for resources, War Machine), used by both gates | core `research_e2::{a_stale_war_machine_mark_does_not_make_a_research_affordable, the_triad_does_not_make_a_research_affordable}`, `production_rules::payable_leaves_out_the_triad_and_for_resources_the_war_machine_faces`; engine lockstep `strategy_research_tests::a_war_machine_mark_that_outlives_its_production_step_does_not_make_the_secondary_affordable_in_lockstep` (pre-fix: counts `(0, 1)`, a bail) |
| 2 | yin 6p s3 (`--first-seed 2`), `CombatUnported` | Brother Milor (the Yin agent, or a Ssruu copying it): "after a player's unit is destroyed during combat: place 2 fighters in the destroyed unit's system". The Yin seat got two Fighter I in round 2 of a fight whose texts (`combat_live::combat_texts_in`) had named only Cruiser I / Destroyer I: the machine stood at a roller it did not name (L's 2d family, a third source of mid-fight units after the upgrade and Reveal Prototype) | string | with Brother Milor held (readied or exhausted) by any seat, every seat in the fight gets the forms of its fighter named (`milor_in_play`, `forms_of`; `forms_of` also replaces L's inline filter of Reveal Prototype) | engine `combat_live::tests::the_texts_of_a_fight_name_the_fighters_brother_milor_can_land_in_it` |

## 2. Sweeps (gate profile, `native_play.sh`, one run at a time, final binary)

* `--factions all --players 3,4,5,6 --seeds 1` (96 games): **96 finish with zero gaps**, 0 stops (int. 15: 94).
* `--first-seed 2 --seeds 3` (288 games): **288 finish with zero gaps**, 0 stops (int. 15: 285).
* No shadow sweep was run (lane P's); `--shadow` first differences of INTEGRATION15 section 6.2 are untouched.

## 3. Mutation (single files, `D4_COVERS_FILE=/dev/null`; the union of set A (core tests binaries) and set B (engine lockstep filters) counts)

| range | mutants | A killed | B killed | survivors in both |
|---|---|---|---|---|
| `kernel/stage.rs` 160-195 (`emit_mirrored`; `emit_mirrored_bare` has no tool mutation point) | 2 | 2 | 0 | none |
| `driver/windows/status_finish.rs` 196-220 (the merged `announce_staged_cards` wrapper, `emit_and_announce`) | 1 compiled (2 not compilable under the switch) | 1 | 1 | none |
| `rules/production/mod.rs` 420-432 (`payable`) | 7 (+1 not compilable) | 6 first run, 7 after `payable_leaves_out...` | 5 | none |
| `rules/production/strategy_cards.rs` 900-910 (`after_offers`) | 4 | 4 | 4 | none |
| `ti4-engine/src/combat_live.rs` 143-310 (`combat_texts_in`, `forms_of`, `milor_in_play`, `researches_in_combat`, `hero_fight_types`) | 40 / 39 | -- | 35 first run, 39 of 39 after the two new tests | none; one equivalent mutant (`forms_of`, `NotInCorpus => false`: the unit is Ghemina's Combat Transport II) is `// mutants:skip` with the reason |

Lines the tool has no point on were mutated by hand (edit, run the union of the tests, restore; every kill read from a `FAILED` test line, not from an error):

| hand mutant | killed by |
|---|---|
| `emit_mirrored_bare` without `mirror_journal` | core tests (set A) |
| `prelude::announce_staged` calls `emit_mirrored` (announcing the card moves early) instead of the bare form | core tests (set A) |
| `status_finish::announce_staged_cards`: `.is_err()` -> `.is_ok()`; `emit_and_announce` -> `Flow::Done` | core tests (set A) |
| `env_of`'s `map: ...` -> `false` | **only** the bridge test `native_play::tests::an_exploration_probe_played_from_the_menu_explores_the_frontier_token_next_to_the_seats_ships` (not the D4 crate, not the engine lockstep sets) |
| `env_of`'s `map: ...` -> `true` | **survives** every set: justified, not skipped by a marker (the tool has no point there): without the Map slice `core.galaxy()` is `None` in a release build (as `false` is) and a debug assertion in a test build; Exploration Probe is a component action and always has the map; whether every window that can play Experimental Battlestation is lent it was not checked (comment at the line) |

J's `held`/`holds` close-order lines were already covered by J's table (`docs/J_RESUME.md`: all killed by set A); not re-run. K's `live/plays.rs` / `reactions_core.rs` and L's other engine ranges (`render/leaders.rs`, `invasion_live.rs`) are still unmutated.

## 4. Checks (cheap), private target `/root/deploytmp/core-q` (profile `lockstep`, debug assertions + overflow checks on for the three core crates, `CARGO_INCREMENTAL=0`; deleted at the end)

* D4 crate (`ti4-engine-core-tests`): 154 result lines, **1951 passed, 0 failed** (int. 15: 1949; +2 `research_e2`; the later `production_rules::payable_...` test, run alone with its binary, passes and makes 1952); `ti4-engine-core --lib` 477, `-factions --lib` 151.
* `ti4-engine --lib` plain and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: **5288 passed**, 0 failed, 7 ignored (int. 15: 5286; +2 tests of sections 1). The two mutation tests added to `combat_live` afterwards (5290 in all) were run with the `the_texts_of` filter (8 passed) and by the 39-mutant run, not in another whole-lib run.
* `check_layers.py core` 574 files 0 violations; `lint_core.sh` 576 files 0 hits; `clippy_core.sh` ok (engine 41 = baseline 41).
* Not run (rules of the lane): golden / wide dual, benches, soak, census, ratchet, workspace clippy beyond `clippy_core.sh`, `end_of_d1.sh`, shadow sweeps.

## 5. What remains

* The shadow matrix of INTEGRATION15 section 6.2 (PB 65, AC 10, NE 9, TG 4, SC 3, S1 3, OT 2) is lane P's.
* Lane N (Jae Mir Kan's waiver with a Nomad) is integration 16; `StrategyAbility` in `strategic/facts.rs` still has that corner (`WaiverEffect::Unported`), no sweep game reaches it.
* Unmutated: K's engine ranges (`live/plays.rs`, `reactions_core.rs`), L's `render/leaders.rs` / `invasion_live.rs`, and every resume note since integration 9 except the ranges above. The heavy gates of INTEGRATION15 section 8 item 6 are all still to run.
* Brother Milor is the only mid-fight source of units the texts name this way; any other unit that appears in a fight (a new faction ability) will show up as a `CombatUnported` in a sweep before it matters.
