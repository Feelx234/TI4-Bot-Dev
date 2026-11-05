# The rest of the golden dual (lane core-dual-rest, 2026-11-05)

Branch `core-dual-rest` from `core-dual6` (`9e887bff8`). Not pushed, not deployed. Goal: all 20 games of `dual_run tools/engine_profile/profile-20.json 20 all=dual` pass (exit 0, fail-fast
off), so that dual-all, dual-drv, dual-wide, dual-lent and strict no longer stop early on a known cause. The probe of the 11 differing games is `docs/DUAL6_RESUME.md` section 2.

**Result: all 20 games pass (section 4).** Every class was root-caused: one is a string bug (S8, recorded in `docs/STRING_ENGINE_BUGS.md`), two were real core gaps (a missing ledger row and a
missing `invasion_seq`, each fixed in the core with a core regression test), and the other seven were gaps of the dual harness itself (lend footprints, one hook missing in the harness
kernel, a menu composed twice, display facts, a recording that told the outer window twice). Nothing is a band-aid: a harness fix lends/reads what the core rule needs, and the one
recognition of a string bug (S8) is narrow, counted, Dual-only and tested. No decision stream of a working game changes: the core changes touch only the ledger rows of an inline Experimental Battlestation play and
the `invasion_seq` of a Jol-Nar commander question inside an invasion; everything else is in `ti4-engine` dual code (`dual*.rs`, `menu_dual.rs`, `core_combat.rs::combat_kernel`).

## 1. The classes, one by one

| class | games | cause | who was wrong | fix | test |
|---|---|---|---|---|---|
| **A** Nano-Forge | 4, 9, 19 | the string never lists `relic|nanoforge` (no `use_relic` arm, not in `action_aliases`) | string: **S8** | on the DUAL run only the core's side is cut down (`relics::action_relics`, `turn::menu.choice`), counted as a compared window `[known string bug S8: ...]` (never a bail, never a difference); the core backend keeps the relic | `relics::tests::nano_forge_is_cut_from_the_cores_list_on_the_dual_run_only` |
| **B** Cabal Vortex menu | 4, 10, 14 | the string lists Vortex in its Mapped source, the core in its Faction source (`menu_faction::faction`: "mapped actions come after every plain one"); `menu_dual` put the string's Mapped ids next to the core's Faction options, so the rendered core menu showed `faction|cabal|vortex` twice. **Not an adjacency question**: both sides computed the same targets in every menu (checked by printing both) | neither (adapter) | `menu_dual::check_action_menu` drops from the string's Mapped part the ids the core already lists natively in the Faction source; a failed component action is dropped by the id of the native option it names (before, only a lent copy of the id) | `menu_dual_tests::cabals_vortex_is_listed_once_by_the_typed_menu` |
| **C** Battlestation dice | 3, 4, 7 | the string notes the three dice of an inline Experimental Battlestation as an ability roll `(Some(5), faces)`; the core wrote a ledger row only for Courageous To The End (`Play::ability_roll`, "Experimental Battlestation is not played from a window", which the games contradict: it is played after the active player's move) | **core** (bookkeeping) | `Play::ability_roll` returns `(holder, faces, hit number)` for Battlestation too; the ledger row has purpose `Ability(BATTLESTATION_ROLLS = 30_002)` and `hits_on` 5 | `action_card_effects::experimental_battlestation_fires_three_dice_...` (now also asserts the ability roll), updated Courageous test |
| **D** Ghost tokens | 10, 12, 13 | Icarus Drive (a reaction to every activation) places a Creuss wormhole token; the window followed the ability but lent and took back only the slices of `ability_fp()`, which lacked `Slice::Explore`, so the core's tokens stayed as before the activation | harness (footprint) | `Slice::Explore` in `ability_fp()` reads and writes | `tactical_dual_tests::an_icarus_drive_inside_an_activation_places_the_token_on_both_sides` |
| **E1** game 7 `tactical::move context.hit` | 7 | the casualty question of the Battlestation's hits (rule 78.4, the active seat asks, the holder produced the hits) lacked the display fact `hit` (cause other, not a destruction, any restriction, producer = holder). The holder can be the active seat itself | harness (display fact) | `battlestation_hit` in `dual_window::drive_positioned`: cause/kind/restriction are the card's, the producer is read off the string's own question (the child window that absorbs the hits does not know who played the card) and must be a seat of the table | `tactical_dual_tests::an_experimental_battlestation_after_a_move_is_followed_in_dice_and_casualty_question` |
| **E2** game 9 `fleet_limit` 4 vs 3 | 9 | a Mahact that wins takes the opponent's command token (Edict) and the enforcement after the win judges by the pool as it is now; the harness's combat kernel (`core_combat::combat_kernel`) did not hold the hook that counts the foreign tokens of a fleet pool (`mahact_foreign_tokens`), so the core's limit stayed at the seat's own tokens (the core's product kernel has it) | harness (kernel) | the hook is pushed in `combat_kernel`; `cc_ground::mahact::query` is `pub` for it | `cannon_prelude_tests::the_end_of_a_fight_won_by_the_mahact_judges_the_fleet_by_the_pool_edict_filled` |
| **E3** game 12 `support:naaz` | 12 | Mentak's commander offers the opponent's own Support for the Throne "while it is home"; the Supports lent out (`support_holders`, `Slice::Trade`) were not lent to the window | harness (footprint) | `Slice::Trade` in `ability_fp()` reads and writes | `cannon_prelude_tests::the_opponent_of_the_mentak_commander_is_not_offered_a_support_that_is_lent_out` |
| **E4** games 12, 14 `finish_control_gain.questions` 1/2 vs 0 | 12, 14 | Naaz-Rokha's Dart and Tai explores the captured planet INSIDE the window (a reaction to `PLANET_CONTROL_GAINED`); the window was lent neither the decks nor the explore state and its table had none of the exploration's words, and the reward question's `commodities` detail is the string's to read | harness (footprint, table, display fact) | `control_gained` unions `exploring_dual::FP`, uses `exploring_dual::table` and `exploring_dual::question_details` when an eligible/run ability is a Naaz one | `game::tests::dart_and_tai_explores_inside_the_control_window_and_the_core_follows` |
| **E5** game 16 `invasion_seq` | 16 | the string names the invasion in progress in every question about its system (`DecisionContext::about_invasion`, battles excepted); the core's Jol-Nar commander reroll question during a bombardment had `invasion_seq: None` | **core** (context) | `kernel::commander_walk::Walk::ask` fills `invasion_seq` from `transient.invasion.active` when it is the question's system | `w16_commander_hosts::the_commanders_question_about_a_bombardment_names_the_invasion_in_progress` |
| **E6** game 19 `factions::mahact::hero` | 19 | the Mahact hero's fight is run inside the hero's recording; the string's `combat::answer` window (composed driver) re-told the outer recording of the question the table had already told, so every sustain question stood twice on the string side (5 against the core's 3) and the core stopped following (`Some(false)`, the position cut short). Neither engine asks twice | harness (recording) | `dual_window::note_asked_before` (and `InsideWindow`, set by `run_dual_window` when it finds a recording running) replaces the six explicit `note_ask` calls of windows whose question was asked before them (`combat::answer`, `invasion::{bombard_answer, answer_custodians, commit_one}`, `ProductionWindow::step`, the tactical step) | `mahact_units::tests::the_hero_fight_on_the_composed_driver_records_each_sustain_question_once` |

Rule rulings: none needed. Where a rule text mattered it was read from the card (Vortex: "a system that is adjacent to 1 or more of your space docks", Nano-Forge, Dart and Tai, Edict,
Experimental Battlestation: "choose 1 of your space docks in or adjacent to the active system ... the active player's ships", so the holder may be the active player). The user's S1
ruling (active player first, then initiative order) is untouched.

## 2. Tooling added to the dual harness

* `TI4_DUAL_TRACE=N`: prints, for the first N differences of a run, the call stack of the comparison (needs a binary with symbols: build with `CARGO_PROFILE_RELEASE_STRIP=false
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`), the option ids of both sides of a differing question, and the question lists (prompt, ids, answer, canonical context, details) of a
  window whose question count differs. This is how the nine harness causes above were found in minutes instead of by reading code.
* `dual::note_known_string_bug(label, id, why)`: a compared window `<label> [known string bug <id>: <why>]` for a recognised, documented string bug (S8 today). The cut itself is the
  caller's and must only apply on `Dual`.

## 3. Checks

* Per-game probes with the instrumented release binary (a private target `/root/deploytmp/dualrest-target`, `CARGO_INCREMENTAL=0`, symbols on): games 3, 4, 7, 9, 10, 12, 13, 14, 16, 19 each went from
  differing to 0 differences one fix at a time.
* **The whole run** (`dual_run tools/engine_profile/profile-20.json 20 all=dual`, 4 shards `--shard i/4 --out`, then `dual_run --merge`, `TI4_DUAL_FAIL_FAST` unset, the final binary):
  `games 20 | hashes equal: true`, `dual: 34626609 calls, 35204353 comparisons, 0 differences`, **exit 0**. No bail label that the probe did not know appeared (the counts grow because game 6 is in
  the run); the new report lines are `relics::action_relics [known string bug S8 ...]` (101 compared) and `turn::menu.choice [known string bug S8 ...]` (101 compared).
* Quick-game fingerprint `538dc68f23dddcc9` (1,165 steps): `cargo test --profile lockstep -p ti4-engine-core-bridge --test wrapper_native` green on the final core, so no decision stream of a working
  game moved (the core changes only add a ledger row to an inline Battlestation play and an `invasion_seq` to a Jol-Nar commander question inside an invasion).
* Tests (`cargo test --profile lockstep`): `ti4-engine-core-tests` `action_card_effects` (26) and `w16_commander_hosts` (8) green; `ti4-engine-legacy --lib` with the filters of this lane plus
  `combat::`, `dual_window::`, `dice::`, `courageous`, `the_use_dispatch_and_the_action_list`, `the_hero_runs_in_lockstep`, `tactical_dual_tests`, `menu_dual_tests`, `cannon_prelude_tests`: 338 passed.
  Not run: the full legacy lib (5,300 tests), clippy, the heavy gates (dual-all, dual-drv, dual-wide, dual-lent, strict) - this lane's goal was to make the dual clean so they can run.
* Mutation check of the new harness tests (each fix reverted in the working tree, tests rerun, then restored): without the fix the tests of A (S8 cut), D (Icarus tokens), E1 (Battlestation
  casualty hit), E2 (Edict pool), E3 (lent Support), E4 (Dart and Tai) and E6 (hero recording) FAIL, and so does B (Vortex listed twice: 4 differences). The first versions of the B and E6 tests
  passed without their fix (the Cabal position lacked the dealt notes so the core view was refused; the unit tests run the composed machine off) and were corrected until they failed.
  C and E5 are core tests asserting the new values (the old code returns `None` / no `invasion_seq`).
* Mistakes found by the tests on the way and fixed (kept here because the same trap waits for the next harness change): the first version of `note_asked_before` skipped every inner-window
  note, which dropped questions the table never recorded (a neutral side answers its sustain question without asking; six `combat::` lockstep tests caught it); it now skips only the
  question the table recorded just before.

## 4. Result

All 20 games pass `dual_run ... all=dual` with exit 0. **Nothing remains** for the golden dual of `profile-20.json`.

What this does NOT cover (so the heavy run can still find things): the other dual configurations of the heavy steps (other seeds/profiles: the probe was the 20 golden games only), the bail
census (`TI4_BAIL_IS_FAILURE`: the S8 cut is a compared window, not a bail, but the 73 other bail labels are what they were).

## 5. Open points and notes for the next lane

* The failed-component-action mapping in `menu_dual` (a failed native option is dropped by its id) has no test of its own: a Vortex that FAILS needs a script the table refuses; none of the 20 games
  produces one. It is a prophylactic part of the Vortex fix.
* `battlestation_hit` copies the producer from the string's own question (the core's absorb window does not know who played the card). If a later lane gives the casualty question a producer
  in the core context, take it from there and delete the copy.
* A harness window that follows nested activity lends `ability_fp()`; every new slice an ability touches must be added there (the next one will surface as `state` differences at the window
  that follows the ability, with the core holding the value from before the window). `TI4_DUAL_TRACE` shows the stack in one run.
* Building with symbols (`CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`) made the dual_run binary 190 MB and the release build 7 minutes incremental (up to 25 under load).
