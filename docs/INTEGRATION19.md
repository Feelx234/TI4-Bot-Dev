# core-integration-19: merge record and shadow round 5 (2026-11-04)

Branch `core-integration-19`, cut from `core-integration-18` @ `1c11e5662`. Two merges (lane S, lane E), then the remaining shadow work of the `--seeds 8` Z-gate command.
Not pushed; the live branch `nightly-fixes-2026-10-26` was not touched; lanes D (`core-d28d-wrapper`) and the silent-gap recompute were not touched.
`RULES_VERSION` stays 35; serde, fixtures, fingerprints (quick game: 1,165 steps, `74d5675ac754ac81`, seen after every change) and wire TS are unchanged.

Read first: `docs/INTEGRATION18.md`, `docs/S_RESUME.md`, `docs/D28E_RESUME.md`, `docs/STRING_ENGINE_BUGS.md` (S3 and S4 are new).

## 1. Merges

| # | Branch | Tip merged | What it brings | Conflicts and how they were joined |
|---|---|---|---|---|
| 1 | `core-s-shadow4` (lane S) | `a0f756da2` | Skilled Retreat in a live game, Mallice/gamma tokens join the core map, the live sustain drains Reflective Shielding hits, no presentation after the head's pause, the Naalu mech printing, S1 for the ability's own prompt, the later-round strategy phase from the speaker, the Nekro agent discard order, the retreat-removal glue | `native_play.rs` (`s1_orders`: S's refactor into `s1_orders` / `s1_orders_differ`, kept, with int-18's status-phase ruling as the third arm), `driver/turn/phase.rs` (**one** `sync_clock` at the round flip: int-18's B6; S's ninth fix was the same line, its comment dropped; both regression tests pass: `the_windows_of_the_flip_into_a_strategy_phase_...` and `the_strategy_phase_of_a_later_round_orders_from_the_speaker_...`), `combat_live.rs` (Q's `forms_of` + Milor kept, S's "ships only" applied to the researched forms; both tests kept), `STRING_ENGINE_BUGS.md` (int-18's closed ruling paragraph kept, S's yssaril paragraph kept, S's "open question" paragraph dropped) |
| 2 | `core-d28e-product` (lane E, P1) | `0fa823be1` | the live branch merge, Thunder's Edge refused at creation / load / replay / resume, old rules versions refused, the nine dropped factions refused, `product_init` | `crates/ti4-engine-next/src/lib.rs` only: lane C's module section and lane E's `// ---- lane E ----` section both kept |

After each merge `cargo check --workspace --all-targets` (without the torch crates) was green, and `awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral` = 0.

### 1.1 Joins that the textual merges did not see (the thin copies)

Lane S was cut before the thin crate existed, so its edits of the legacy crate had to be made again in the thin copies the native loop drives (the rule of INTEGRATION18 section 8.2):
`ties/combat_live.rs` (ships only for the researched forms), `ties/core_adapter.rs` (`Galaxy::map_wormhole_kinds` + the tokens apart), `render/combat.rs` (a `Withdraw` answer is the question's own option), `live/mod.rs` (the test). Found because the bridge test
`the_cards_of_a_combat_round_window_play_in_a_live_game_as_the_string_plays_them` failed on the merged tree (winnu 4p s4 step 415: Skilled Retreat's destination pick); it passes after the port.

### 1.2 The user ruling, and S's report

The status-phase ruling of 2026-11-03 (windows with no active player: clockwise from the seat after the speaker, the speaker last) is what the tree implements. S's note "kept as is (LRR 1.19)" does not win: `STRING_ENGINE_BUGS.md` carries the ruling and S's text was dropped.
`resolver_basics::a_status_phase_window_without_an_active_player_goes_clockwise_after_the_speaker_and_the_speaker_last` passes, and `native_play::s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ` (speaker second seat differs, speaker last does not) passes.
The harness reports the mahact 6p seed 1 case as the known S1 (the games of the sweeps below count it under S1).

### 1.3 S's one changed D4 assertion, re-justified

`aftermath_chain::an_empty_system_presents_the_finished_fight_unless_the_head_paused`: the case `(1 ship, paused)` went from `presented = true` to `false`. **Kept.** The string is the reference and the case is its behaviour on a real game (jolnar 5 players seed 1: the defender's guns destroyed the
activator's only ship in the head's space cannon offense, a side still held the system, and the string presented nothing: its `AftermathWindow::new` does not settle the combat window it built, because after the pause the window is already done). The old `true` had been derived from the core, measured only on synthetic
positions (lane R). Nothing was loosened: the other three cases of the table are unchanged and the message of the last assertion is the only text edit. Re-run in this tree: passes.

### 1.4 `host_create_golden` (lane E x lane C): one assertion changed, the golden file is byte-identical

`tests/host_create.golden` (1,405 digests, recorded at the legacy setup) is not edited. With lane E, Thunder's Edge is refused at creation, so 302 of its rows (252 plain tables with `te=true`, 50 `expedition` presets) were `ok:` games at the legacy setup and are `err:` now; "passes unchanged" and "TE stays refused" cannot both hold for the old assertion.
The test now states it: for those 302 rows (`refused(line)`) the golden row is a game and the new row is an error (and the count is asserted to be 302); every other row (1,103) must equal the golden exactly, and the "most of the matrix creates a game" floor is kept. Without lane E's refusal the new assertion fails (the 302 rows would be `ok:`).

## 2. Bugs found and fixed in this round (every one has a test that fails on the pre-fix code, see section 5)

| # | Class (games) | Cause | Right | Fix | Guard |
|---|---|---|---|---|---|
| 1 | `CombatUnported` gaps (hacan 6p s8, arborec 6p s8, arborec 4p s7) | Traced with an instrumented `Machine::run`: the `None` came from `open_round`'s `every_roller_is_named`. The unit texts (`combat_texts_in`) named the units on the PLANETS of the system as well (infantry, PDS, dock of the seats there) and each kind named doubles the roll groups; with the other forms (`other_form`) the fight went over `MAX_TYPES` (10), which names none (12, 12 and 11 kinds). A ground force or a structure rolls no die in a space combat | core | planet units are named only where they can fight as ships: a ship, an Eidolon Maximum, or a ground force of a Nekro whose Alastor stands in space or has chosen the unit (`fights_as_a_ship`: the Alastor test `the_alastor_asks_per_planet_and_unit_...` caught a first version that looked at the flagship only, the mark outlives it) | legacy `combat_live::tests::the_texts_of_a_fight_name_the_ships_and_not_the_ground_forces_and_structures_on_the_planets` (fails on the old code: no kind named) and bridge `a_fight_with_ground_forces_and_structures_on_the_planets_names_its_ships` (the three games) |
| 2 | nomad 3p s5: `sustain\|2` | the live kernel (`live::kernel::kernel_for`) lacked the legacy slot `combat.grants_sustain`'s Nomad entry: the string's combat kernel fills it with `register_naaz_nomad`, the faction module registers the seam only, and `sustains_in_space` reads the slot. A Nomad mech (Quantum Manipulator, "while in a space area during combat you may use its SUSTAIN DAMAGE") was never offered. A **product bug**, not only a shadow one | string (card text) | `hooks.combat.grants_sustain.push(nomad_grants_sustain)` in `kernel_for` | thin `live::kernel::tests::the_live_kernel_lets_a_nomad_mech_in_the_space_area_sustain` and the game in bridge `the_nomad_mech_sustains_in_space_and_war_effort_lists_systems_with_ships` |
| 3 | nomad 6p s6: option 11 vs 23 (War Effort) | the core listed `systems_with_units_of` (ANY unit of the seat in the space area), the card says "a system that contains 1 or more of your ships"; the Saar's Floating Factory stands in the space area, "can move and retreat as if it were a ship" but is a structure (`ship: false`). The core's own `playable` check already asked for ships | string | the options are the systems with a ship of the seat | D4 `action_card_effects::war_effort_does_not_list_a_system_that_holds_only_a_floating_factory` |
| 4 | naalu 4p s8, ghost 4p s6 and 14 more games of the 8-seed set (strategy token, trade goods) | the harness' own: a native step announces what the step before staged FIRST (its reconcile), the string's step ends with the arm's first events (`INVASION_BEGAN`, the commit question) and announces at the start of the NEXT step, or after the whole action when no question is asked. A product session splits at every question, so the order of the core is the string's; the shadow's step holds the question and its answer. A first version only skipped the reconcile less often (`handed_off` now needs `asked.is_empty()`, as its comment said) and exposed more games | harness (the core's order is the product's) | the events a reconcile announced are compared by value, not by position: `Owed::take_announced` / `strip_pending` (every other event is compared in order; the pending list is also taken off the `staged` comparison) | bridge `a_token_the_movement_window_staged_is_announced_behind_the_invasion_the_string_began` (naalu, ghost; the unit part of the helpers) |
| 5 | yssaril 3p s5 | S1 in the legacy-error form where the script's answer is the ability's OWN answer (`exhaust` of Suffi An), which `order_in_error` did not recognise | ruling (S1) | `order_in_error` accepts `exhaust` | bridge `the_string_bugs_of_integration_19_are_recognised_and_nothing_else_is` |
| 6 | empyrean 6p s5 (`won_against_a_note_holder`) | **string bug S3**: see `STRING_ENGINE_BUGS.md` | core | none (recorded, harness recognises `s3_feat_differs`) | the same bridge test |
| 7 | yin 4p s8 (`82a\|lockedmallice`) | **string bug S4** (the Yin hero's planets walk the hex grid, the Nexus sits beside it): see `STRING_ENGINE_BUGS.md`; **question Q1 below** | core, by the card text | none (recorded, harness recognises `s4_nexus_in_error`) | the same bridge test |
| 8 | the three `TurnGap(StrategyAbility)` gaps (l1z1x 4p s3, xxcha 4p s5, jolnar 4p s8: Jae Mir Kan) | **not reproducible in this tree**: all three games finish with zero gaps (jolnar 4p s8 ends at a known S1). The gap was lane N's waiver area, which is on this line since integration 17/18 | -- | -- | the three games are in the sweeps; no new test (nothing was changed) |

### Questions for the user

* **Q1 (S4)**: Dannel of the Tenth commits infantry to "any non-home planets". The core offers the Wormhole Nexus' planet too (`82a|lockedmallice` while locked, `82b|mallice` open): it is a planet, not a home planet, and the same game has a seat using its legendary ability. The string never offers it (its list walks the hex grid). If the Nexus is
  not meant to be a target while it sits beside the board (or while it is locked), the core's `driver/turn/hero.rs::yin_spots` needs one more line and the harness recognition goes. Default if you say nothing: the core's reading stays.

## 3. Sweeps (plain and shadow), final binary

### 3.1 The whole game, plain (`native_play.sh`, gate profile, `te` card set)

| run | games | finished with zero gaps | native steps | notes |
|---|---|---|---|---|
| `--quick` (sol, 4 players, seed 1) | 1 | **1** | 1,165, 9 rounds, fingerprint `74d5675ac754ac81` | unchanged from int. 17 and 18 (seen after every change of this round) |
| `--factions all --players 3,4,5,6 --seeds 1` (the 96-set) | 96 | **96** | 102,992 (int. 18: 103,015) | S's fixes, ours |
| `--factions all --players 3,4,5,6 --first-seed 2 --seeds 3` (the 288-set) | 288 | **288** | 298,270 (int. 18: 298,315) | |

### 3.2 The shadow, and the Z-gate command

The Z-gate command `native_play.sh --factions all --players 3,4,5,6 --seeds 8 --shadow` was run as four processes (one per `--players` value, two at a time; the 768 games are the same, the exit code of the single command is the conjunction of the four): **all four exit 0**.

| set | games | finished, zero gaps | whole game, no difference | first difference of the core | known string bug |
|---|---|---|---|---|---|
| seed 1 (the 96-set, shadow) | 96 | 96 | 83 | **0** | 13 (12 S1, 1 S2) |
| seeds 2 to 4 (the 288-set, shadow) | 288 | 288 | 248 | **0** | 40 (26 S1, 14 S2) |
| seeds 5 to 8 | 384 | 384 | 328 | **0** | 56 (35 S1, 19 S2, 1 S3, 1 S4) |
| **seeds 1 to 8 (the gate)** | **768** | **768** | **659** | **0** | **109** (73 S1, 34 S2, 1 S3, 1 S4) |

(The 96-set and 288-set rows are cut out of the gate run by seed: the same binary. Lane S's report for the same sets: 83 / 0 / 13 and 247 / 0 / 40, and for the gate 761 finished, 7 gaps, 7 differences, 104 known: the 7 gaps and 7 differences are gone, see section 2.) A game that stops at a
known string bug is played to its end by the core alone (that is what the "finished" column counts); the shadow just cannot compare past the bug.

Per class of the S report: the 3 `TurnGap(StrategyAbility)` gaps (not reproducible, section 2 row 8), the 4 `CombatUnported` gaps (row 1, three games; the fourth, saar 6p s8, plays clean in this tree), and the 7 differences (rows 2 to 7: naalu 4p s8 and ghost 4p s6 row 4, yin row 7, empyrean row 6,
nomad 6p s6 row 3, nomad 3p s5 row 2, yssaril 3p s5 row 5) are all closed or recorded. The gate has **no difference of the core left**.


## 4. Remaining differences

* None of the core. Every stop of the shadow is a known string bug: S1 (73, the order of the reactions of one event, with the ruling of 2026-11-03 for the status phase), S2 (34, the windows of the flip into a strategy phase), S3 (1, empyrean 6p s5) and S4 (1, yin 4p s8). The harness counts them apart and leaves them out of the exit code.
* Question Q1 (section 2): S4 depends on whether the Wormhole Nexus may be a target of Dannel of the Tenth.
* The `announced` events (section 2 row 4) are compared by value, not by position: a real mis-ordering of a reconcile's announcement against the surrounding events is not seen by the shadow any more (the order of the product is the core's, covered by the D4 tests of the reconcile and by the lockstep tests; the strings step boundaries are what the shadow cannot place).
* Not done, as in int. 18: the heavy gates (section 6), the ratchet, the mutation batches (lanes N, P, Q, R, S, A, B2, C, E and this round), the `Game` wrapper (lane D).


## 5. Fail-before evidence

Run in place: the production hunks of this round (the five fixes, not the tests) were reverted in the tree, the named tests run on that tree with the same profile, then the tree restored (`git checkout`). All fail, for the reason they guard:

| Test | Result on the pre-fix code |
|---|---|
| legacy `combat_live::tests::the_texts_of_a_fight_name_the_ships_and_not_the_ground_forces_and_structures_on_the_planets` | FAILED (no kind named) |
| thin `live::kernel::tests::the_live_kernel_lets_a_nomad_mech_in_the_space_area_sustain` | FAILED |
| D4 `action_card_effects::war_effort_does_not_list_a_system_that_holds_only_a_floating_factory` | FAILED |
| bridge `a_fight_with_ground_forces_and_structures_on_the_planets_names_its_ships` | FAILED: hacan 6p seed 8 `CombatUnported` |
| bridge `a_token_the_movement_window_staged_is_announced_behind_the_invasion_the_string_began` | FAILED: naalu 4p seed 8 step 671 `GROUND_COMMITMENT_FINISHED` against `STRATEGY_TOKEN_SPENT` |
| bridge `the_nomad_mech_sustains_in_space_and_war_effort_lists_systems_with_ships` | FAILED: nomad 3p seed 5 step 832 `decline` against `sustain\|2` |
| bridge `the_string_bugs_of_integration_19_are_recognised_and_nothing_else_is` | FAILED: yssaril 3p seed 5 step 711 not recognised as S1 (the first assertion of the test; its S3 / S4 games and recogniser cases were not reached on that run) |

(A first attempt left `take_announced`'s drain in place and failed at step 13 of every game: not the failure that is guarded; the reported run reverts the whole function.)


## 6. Checks (cheap), private target `/root/deploytmp/core-i19` (profile `lockstep` for the tests, `gate` for `native_play`; `CARGO_BUILD_JOBS=2`)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without the torch crates) after each merge and at the end | ok; `settle_run` neutral-side grep **0** |
| `ti4-engine-legacy --lib` (plain, the whole suite) | **5299 passed**, 0 failed, 7 ignored (int. 18: 5295; +S's 2, +ours, +the new texts test) |
| thin `ti4-engine --lib`; `--test wire_parity`, `ties_parity`, `surface_parity` | 364 (+1 ignored); 11, 6, 3 passed (after merge 2) |
| bridge `--lib` | **140 passed** (int. 18: 130; +S's 6, +4 new here); `--test host_create_golden` ok (section 1.4, golden file untouched) |
| `ti4-engine-core --lib`, `-core-factions --lib` | 476, 153 passed |
| D4 binaries `action_card_effects`, `action_cards`, `aftermath_chain`, `combat_machine`, `resolver_basics` (lockstep profile) | 26, 11, 24, 27, 7 passed (the full 154-binary D4 run was not repeated: the core crates changed by one function, `Fx::WarEffort`'s options) |
| `ti4-host` `--lib`, `refusals`, `creation_guard`, `product_assertions`; `ti4-server --lib`; `ti4-wasm --lib` | 310, 3, 2, 3; 58; 9 passed (after merge 2) |
| `check_layers.py core`, `lint_core.sh` | 574 files 0 violations; 576 files 0 hits |
| `surface_paths.py --check` | the ratchet only shrinks: lane E's host call sites now resolve to 14 more native thin paths than the baseline held (`builders::promissory::*`, `core_sync::unsupported::find`, ...); `--update` rewrote `tools/engine_core/surface_paths.baseline.json` **43 -> 19 legacy** (197 paths), check ok |
| `tools/engine_core/clippy_core.sh` | ok (engine 41 = baseline 41) |
| `native_play.sh --quick` | 1 game, 1,165 steps, fingerprint `74d5675ac754ac81` |
| the sweeps | section 3 |
| target size, disk | `/root/deploytmp/core-i19` 4.3 GB at the end; `/` had 38 GB free |


Not run (user rule): the heavy gates (golden / wide dual, benches, soak, census, ratchet, workspace clippy), mutation batches, `TI4_CORE_STRICT=1`, Playwright, `cargo wire-web`, the torch crates.

## 7. Tip

`git log -1 core-integration-19`. The private target directory and the logs under `/root/deploytmp/i19-*` are removed at the end.
