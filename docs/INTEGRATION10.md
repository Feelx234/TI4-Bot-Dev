# core-integration-10: merge record (2026-10-31)

Branch `core-integration-10`, cut from `core-integration-9` @ `d23867ff`. Five merges, the hand-over of the production step between lane M5 and lane M2,
cheap checks only (the user rule: heavy work waits for a full zero-bail playthrough). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched.
Not merged, by order (still being worked): `core-m6b-tactical`, `core-w-l16`, `core-d2-c1-status`, `core-d2-f2-hooks`.

## 1. Merges

| # | Branch | Tip merged | What it brings | Conflicts |
|---|---|---|---|---|
| 1 | `core-d2-f-live2` | `bfbab082` (contains `core-d2-f-live`, `core-d2-l1`) | the live loop (`driver/live.rs`, `live/*`), `NativeSubject` (the shadow oracle), play facts (`live::plays`), `playthrough --quick`, `window_driver::driver_for` (the one constructor of an adapter's window driver; a guard test forbids a hand-built `Driver::new(` in production code), `Pending::resolver` | 3 files |
| 2 | `core-m2-production` | `caf2d2d3` (contains `core-x-explore`, `core-m3-invasion`, the `core-w-l14` chain and `f-live2`) | lane M2: one use of PRODUCTION as a held window (`driver::windows::production`, `Arm::Production`, `UseOpen::{Tactical, Home, Ability}`), `ProduceOne` in the state layer (`OneLink`), Freelancers and Titans' Terragenesis (a rule of the core table, slot 17001), the hand-limit discard in a hosted exploration; lane X: the exploration as one core window (`explore_windows::Exploring`, `ExploreRun`), the Empyrean hero through it; lane M3: the invasion as a held window (`driver::windows::invasion`, `Arm::Invasion`) | 7 files |
| 3 | `core-m5-aftermath` | `b15c8773` (contains `core-m6-tactical`) | lane M6: the tactical action (`driver::windows::tactical`, `Arm::Tactical`); lane M5: the aftermath chain (`driver::windows::chain`, `Arm::Aftermath`), the space combat as a window (`windows::combat`, the machine in LIVE mode), `EventScoring` (`Arm::EventScoring`), the staged-destruction drains, Van Hauge as a rule of the core table | 8 files |
| 4 | `core-d2-c2-turn` | `fe3e3b85` (lane B) | the turn flow: `Window::TurnFlow` (reconcile, prelude, finish, end, phase, announce, draft jobs), `Arm::{TurnPrelude, StrategyAnnounce, Phase, Ask}`, `live_reconcile`, `Asking.at` | 8 files |
| 5 | `core-w-l15` | `ba43ce6a` | Courageous To The End and Scramble Frequency played inside a window, the hosts' staged dice (barrage, space cannon offense, bombardment `BombardRolled`), the neutral-side scenarios, the Cabal participation mark | 1 file (docs) |

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine/src/hero_dual.rs` (1, 2) | (1) f-live2 builds every driver with `window_driver::driver_for`, int9 added the Naaz/Winnu facts (`perform_facts(.., galaxy, .., &list)`, `drive_positioned`); (2) the Empyrean hero: lane 14 builds it with `FrontierFacts`, lane X replaced the facts by the exploration window and a table (`exploring_dual::table`, `self_assembly::register_links`) | (1) int9's facts and `drive_positioned`, built with `driver_for`; (2) lane X's version (git had already taken lane X's `hero_windows.rs`: `EmpyreanHero::new(seat, placed, order)`), `driver_for` |
| `ti4-engine/src/reactions_core.rs` (1, 2, 3, 4) | f-live2 moved the table codes into `set_table_codes`; int9 added `codes.indoctrination`, M2 `codes.terragenesis`, M2 `production_live::fill_codes`, M6 `codes.tactical`, B `codes.turn` | one `set_table_codes` with every line |
| `ti4-engine/src/strategy_cards_core.rs` (1) | `drive_window_on(kernel, ..)` (int9, the Politics draw runs on the full table) against `driver_for` | both: `drive_window_on` built with `driver_for` |
| `ti4-engine/src/dual_window.rs` (2, 3, 4) | `_sources` unused parameter of `build_settle_env` (int9 clippy fix) against `sources`; the `Unknown` words: `ReflectiveShielding` (deleted by W11), `UnportedTactical`, `UnportedAftermath`, `Unported`, `UnportedProduction`, `TurnGap` | `_sources`; every new word kept, the deleted one stays deleted |
| `ti4-engine/src/cannon_dual.rs`, `promissory_windows.rs` (2) | int9's doc / Mageon fixture against the older copy in M2's base | int9's (the Mageon fixture is the Nekro/Yssaril one of integration 9, B3) |
| `docs/ENGINE_KNOWN_ISSUES.md`, `docs/LENT_CENSUS.md` (2, 5) | M2's base carried the old `W11-/W12-/W14-design-NN` rows again (the ids int9 renamed to `W11b-/W12b-/W14b-`) next to the new X rows and the `W15` section | int9's renamed rows, the X rows (`X-design-01/02`, `X-gap-01/02`) and the Package X census paragraph, the W lane 15 section |
| `ti4-engine-core/src/kernel/ability.rs` (3, 4) | `Unknown` and the gap enums: M2 `UnportedProduction(ProductionGap)`, M5 `UnportedAftermath(AftermathGap)`, M6 `UnportedTactical(TacticalGap)`, M3 `Unported(Unported)`, B `TurnGap`; `Codes` (`tactical`, `invasion`, `explore`, `production`, `turn`); `AdapterFacts` (`explore_words`, `combat: CombatTexts`) | all of them; M5's side still named `ReflectiveShielding` (deleted by W11): not kept |
| `ti4-engine-core-factions/src/cd_registry/yin/timing.rs` (3) | int9: Indoctrination (slot 7003) and Greyfire Mutagen (7004); M5: Van Hauge (**also slot 7003**) | all three abilities; Van Hauge is `Slot(7005)` (see B2) |
| `ti4-engine-core/src/driver/live.rs`, `driver/step.rs`, `driver/windows/mod.rs` (3, 4) | the arm registry (`live_arm`, `ask_arm_fn`) and the registry test | every arm of every lane; the test names the ten registered arms (section 4) |
| `ti4-engine/src/live/mod.rs` (3, 4) | `render_asking`: M2 renders with the position (`Typed::new_at`) and M5 gave `complete` the kernel and the nested position, B added `Asking.at`; `live_step_inner`: M5 prepares an `Env` with the combat texts, B takes the arm's two functions as parameters | `at = asking.at or asking.nested's position or core`, the position and the kernel handed to `complete`; `live_step_inner(core, advance, ask, ..)` that prepares the `Env` |
| `ti4-engine/src/game.rs`, `render/strategy.rs` (4) | adjacent additions (`production_step_tests` / `turn_live_tests`; `PdsAlternative` / `Draft`) | both |

## 3. Bugs and breaks found, and how each is guarded

"Fails before" says how the guard was seen to fail (R: a run on the code before the fix; B: by construction, the removed code is what the test pins).

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | `window_driver` (merge 1) | int9's lanes 9 to 14 built five drivers by hand (`dual_window.rs` x3: ForcedCombats, MahactHero, YinHero; `hero_dual.rs` x2), which the armed flag cannot reach | `driver_for` | `window_driver::tests::no_adapter_builds_a_driver_without_the_constructor` (existing; B: it lists exactly these lines) |
| B2 | `cd_registry/yin/timing.rs` (merge 3) | **a slot clash that merged without a conflict**: M5's Van Hauge and int9's Indoctrination are both `Slot(7003)` (a second registration is a `debug_assert` in every table build) | Van Hauge is `Slot(7005)` | `the_yin_slots_are_distinct` and `the_whole_yin_table_registers_without_a_slot_clash` (new, `timing.rs`; R: both fail with Van Hauge on 7003, `slot Slot(7003) registered twice`, and pass on 7005) |
| B3 | `driver/windows/combat/env.rs` (merge 3) | `Env::show_neutral` (W11) is a field M5's live `env_of` did not know: it did not compile | `show_neutral: false` (the live fight answers the neutral force by the rules and never puts its decision as a question) | the compile; the neutral scenarios of `combat::tests` |
| B4 | `driver/live.rs` + three custom arm drivers (merge 4) | B added `Asking.at` (the position a nested question was put in; the display facts read it); the invasion, tactical and aftermath arm drivers (`windows/{invasion,tactical}/live.rs`, `live_common.rs`) build `Asking` by hand and would have left `at` empty | `asking_of` is `pub(crate)` and all three use it | the compile; `combat_live::tests`, `invasion_live::tests`, `tactical_live::tests` read the nested position through it |
| B5 | `live/env.rs` `env_with_plays` | **a real bug found by lane M2 and left for lane A2**: the live `Env` names the faction units of the FULL scope (Thunder's Edge reprints included) whatever the game's source set is: a Prophecy of Kings game builds the Naalu `naalu_mech_te` in a production step and an Assimilate seat places the reprint, the string names `naalu_mech` | the roster is `roster_for(embedded, sources)` of the game | `combat_live::tests::the_live_env_names_the_faction_units_of_the_games_own_scope` (new; R: with the roster line removed it fails `left: Some(IconoclastTE), right: Some(Iconoclast)`); the M2 tests that overrode the roster stay green |
| B6 | `rules/combat` dual test (merge 3) | `a_yin_seat_that_uses_devotion_keeps_the_composed_machine_on_the_string_and_says_why` (lane 15) pinned the bail on a producer-assigned hit that lane M5 closed; its own doc says "fails the day the machine carries it: then this test becomes a bail-as-failure test and `yin` joins the rotation" | see section 6 | R: the old assertion `!composed_keys.is_empty()` failed in the engine lib run of this branch |
| B7 | `ti4-engine-core-tests/tests/w15_scramble_hosts.rs` | the stub card of `the_reroll_window_of_a_bombardment_emits_the_event_with_the_staged_hits_and_reads_the_faces_back` counted the PRINTED faces against the event's hits while the test's own last assertion says a 2 with a +3 is a hit: **the test fails on `core-w-l15` itself** (run on a scratch worktree of `ba43ce6a`: left 3, right 2); the code under test (`BombardRolled::staged_hits`) is right | the stub counts the adjusted faces with the same formula | R: it failed on `ba43ce6a` and on the merge; green with the stub fixed. The assertions about the window are untouched |
| B8 | `ti4-engine/src/turn_live_tests.rs` (merge 4) | `content_ids_resolve` (the lint over the engine sources; it skips only `*_tests/` directories and `fixtures.rs`) scanned lane B's test file: three `.expect("placed")` on lines with `SystemId::new(` are id-shaped literals that match nothing in the corpus. (Lane B never ran that test.) | the message reads `"a placed planet"` | `content_ids_resolve::every_hardcoded_content_id_resolves_in_the_corpus` (existing; R: red on the merge with the three lines, green after) |
| B9 | `ti4-engine/tests/decision_delivery_inventory.rs` (merge 1) | **`core-d2-f-live2` never ran this test**: the live loop (`live/mod.rs`) asks an `Asker` with the rendered question and no view (`live_step_inner`: `asker.ask(..)`, `LoneCard::ask`: `self.0.ask(q)`); two unreviewed `AskViewless` sites, so `every_producer_and_delivery_site_matches_the_reviewed_registry` and `the_remaining_viewless_asks_stay_explicit_migration_work` failed (`cargo test` stops at the first failing binary, so the first two chain runs of this branch had not reached it) | the two sites are in `VIEWLESS_ASKS` (explicit migration work: the `Asker` is the host's seam, the product wrapper of D2-8 delivers with the view of the asking seat); the ratchet counts 1 -> 3 (registry sum and scanned total) | the two tests above (R: red on the merge with `Some(1), None` for each site; green with the sites reviewed; the ratchet was RAISED by exactly the two sites, and a third new viewless ask trips it again) |
| B10 | clippy gate (`clippy_core.sh`) | none of the lanes M2, M3, M5, M6, B ran clippy: unused imports (`Asking`, `System`, `Faction`, `Ctx`), an unused `who`, `contains` for `iter().any`, two "very complex type" in the core (`PlaysProvider`, `OneLink`) and two in the engine tests, six `mut`/unused parameters | `PlaysFn` and `BuildableFacts` aliases, `LiveRun` and `HullKey` aliases in the tests, `_`-prefixed parameters of `defense_unported` | `clippy_core.sh` (see section 7) |
| H1 | **the hand-over** | the aftermath's `Producing` stage was the counted gap `AftermathGap::ProductionStep` while the seat had a production capacity | section 4 | section 4 |

No assertion was loosened to make a test pass; the edits of assertions and fixtures are listed in section 6.

## 4. The hand-over M5 -> M2 (the production step of the aftermath)

Lane M5 ended the chain with `AftermathChain::producing` returning `AftermathGap::ProductionStep` whenever the active seat had a PRODUCTION capacity in the
system; lane M2's `Arm::Production` / `UseOpen::Tactical` is the native production step (`enter_production`: the capacity-zero case, the number, the Hacan
hero offer, Sarween Tools / AI Development Algorithm, `PRODUCTION_USED`, the window, `finish_use` with `UNITS_PRODUCED`, Self-Assembly Routines). Now:

* `state::Aftermath::produced` (LIVE, like `invaded`): the use was opened. The string-observed copy leaves it false; `d1_action::aftermath_back` (the compared key) does not read it.
* `chain::producing`: a `Window::Production` on the stack parks the chain; with `produced` unset and a capacity above zero the chain opens
  `open_production(UseOpen::Tactical { who, system })`, sets `produced` and parks; once the use has left the stack (or when the seat has no capacity, as before) the step
  announces `PRODUCTION_RESOLVED` and the chain is done. The capacity-zero case still asks nothing and takes no number (M2's `Tactical` opener does the same if
  opened, but the chain does not open it).
* `advance_aftermath` / `ask_aftermath` hand the answers and the question of the step to the production use while `Window::Production` is the top sub-window, as they do
  for the combat and the invasion (`Arm::Production` itself is still not chosen by `step_arm`; `NativeSubject` cannot name it, section 5).
* `AftermathGap::ProductionStep` is deleted (and its words in `combat_live::gap_text`): a position with a capacity is native or fails with the use's own gap
  (`Unknown::UnportedProduction(ProductionGap)`).

Tests (bail is a failure in all of them):

* core, `ti4-engine-core-tests/tests/aftermath_chain.rs`: `a_production_capacity_hands_the_step_to_the_production_use_and_announces_it_resolved` (the question
  is the use's, the chain waits under it, `PRODUCTION_RESOLVED` is not written while the use asks and once after it; `production_seq` is 1; the emitted stream starts
  with `PRODUCTION_USED`), `a_unit_built_in_the_aftermath_production_step_is_announced_before_it_is_resolved` (a destroyer is built and paid; `UNITS_PRODUCED` is
  emitted; a clone in the middle of the use asks the same question), `a_system_without_a_production_capacity_ends_the_step_at_once`. They replace
  `a_production_capacity_is_the_counted_gap_of_the_production_step` (that test asserted the gap).
* engine, `combat_live::tests` (lockstep against the string `Game`: questions with player, prompt, options, details and context, the events of the aftermath's arms in
  order, the whole position):
  `an_undefended_system_with_a_space_dock_goes_on_to_the_production_step_in_lockstep` (ten factions: sol, hacan, arborec, letnev, mentak, yin, jolnar, naalu, nekro, muaat;
  the use declined, and one destroyer bought), `a_won_space_fight_over_a_space_dock_goes_on_to_the_production_step_in_lockstep` (sol, jolnar, xxcha x 3 seeds, a
  fight of dreadnoughts against a cruiser, then the production step with a buy), `a_won_invasion_with_a_space_dock_goes_on_to_the_production_step_in_lockstep` (sol, hacan,
  yssaril, xxcha, winnu; the infantry takes the planet that has no dock, the dock planet is the invader's, a buy). The harness answers the production questions by a
  budget of builds (`default_answer`), the same on both sides.
* Left out of those sweeps, with the reason (each is a counted residual of another lane, not of the hand-over): Saar (the Saar agent's `SYSTEM_ACTIVATED` ability is
  registered by the string and missing from the core registry: M6 open item 2), Nomad (Mercer on `TACTICAL_ACTION_ENDED`: same, and the native tail does emit the event; the
  string asked three more questions and exhausted the agent), Titans (`TacticalGap::ActivationHook`: M6 open item 1).

## 5. Arms, registries, `NativeSubject`

* `Arm` has 21 variants (`Arm::ALL.len() == 21`, `NativeArms::ALL` is 21 bits). The merged registry names TEN arms with a live function:
  `Tokens, EventScoring, Aftermath, Tactical, Invasion, Production, TurnPrelude, StrategyAnnounce, Phase, Ask`
  (`step.rs::tests` asserts `NativeArms::registered()` equals exactly that set; the lanes had asserted 3, 5, 5 and 6 of them). `live::arms::{name, parse}` names all 21
  (the test that the names are distinct and read back is lane F2's, unchanged). Arms that `step_arm` can return: Tokens, EventScoring, Aftermath, Tactical, TurnPrelude,
  StrategyAnnounce, Phase, Ask. Arms run by NAME by a composer: Invasion and Production (by the aftermath chain; the invasion also by the tactical composer's commit step).
* Wiring check of `NativeSubject::open_arm` (one line per native arm that `step_arm` can return; "no opener" counts the step as lent):

  | arm | `open_arm` line | why / what is missing |
  |---|---|---|
  | Tokens | yes (f-live2) | |
  | TurnPrelude, Phase, StrategyAnnounce, Ask | yes (lane B: the jobs open themselves) | `Ask` only for the draft and the starting technology (`live_ask_arm`) |
  | EventScoring | **no** | the observed window (`Window::EventScoring`) is a plain payload and could be opened with one line; not added: no shadow test exists for it and the line would run unproven in the oracle. The lockstep `combat_live::tests::a_capital_ship_destroyed_opens_the_scoring_window_and_scores_in_lockstep` is the proof of the arm |
  | Tactical, Aftermath | **no** | the string holds the combat / invasion inside its `AftermathWindow` (`Aftermath::Fighting(window)`): the observed copy stands at the stage but has no `Window::Combat` / `Window::Invasion` to run, so a shadow open would fail at once; needs the observed sub-windows of D1 (`d1_pending::observed` would have to type them). Proven by the lockstep tests against `Game` |
  | Invasion, Production | n/a | not chosen by `step_arm`; reached through Aftermath |

  Result: every native arm that the oracle could name has its line or a reason; the three arms of the aftermath family (and Tactical) stay "no opener" in `--native`.
* `Facts::open` bits: `WindowKind` has 19 variants (Combat, Invasion, Production, TurnFlow among them), all below bit 32.

## 6. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `driver/step.rs` registry test | three-/five-/six-arm sets replaced by the union of ten; 21 bits | the lanes registered different arms; the union is the registry |
| `combat::tests::a_yin_seat_that_uses_devotion_...` | renamed `..._fights_through_the_composed_machine_in_lockstep`, now asserts compared > 0, no bail (single windows AND composed, plain and neutral garrison); `yin` first in the rotation of `a_neutral_side_fights_in_the_core_for_every_faction_across_from_it` | the test's own doc ("fails the day the machine carries it: then this test becomes a bail-as-failure test and `yin` joins the rotation"); lane M5's producer-assigned casualty closed it (B6). The comparison with the string passes in both modes, so it is not a vacuous pass |
| `w15_scramble_hosts.rs` stub | counts the adjusted faces | B7 (fails on `core-w-l15`) |
| `aftermath_chain.rs` | the gap test replaced by three tests; `env()` carries the production table (roster, `codes.production`, Self-Assembly links); two unused `mut`s and the unused `SHIPS` removed | section 4 |
| `combat_live::tests` harness | `default_answer` (production / pay / load questions after the script, the same on both sides); the `#seen:` marks of the string's reconcile hooks are removed from `faction_marks` of both position diffs | the existing filter dropped differences whose PATH names a `#seen:` mark, but a map holding nothing else (a Yssaril table) is absent on one side and present on the other, a difference at the map's own path: the filter's intent applied properly. The marks are the string's first step (`live_reconcile` of lane B writes them natively) |
| `turn_live_tests.rs` | three `expect` messages | B8 |
| `defense_unported` | `_gs`, `_content`, `_sources` | the parameters are unused since lane X lifted the gates; the callers are unchanged |

Fixtures kept from integration 9 on the merge of `promissory_windows.rs` (`mageon()` seats a Nekro holder when the hand limit applies; the Yssaril guard assertion).

## 7. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int10` (profile `lockstep`, no incremental)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor), after each merge and at the end | ok |
| `cargo test -p ti4-engine --lib` | 5019 passed, 4 ignored |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (whole lib) | 5019 passed, 4 ignored |
| `ti4-engine-core --lib`, `-bridge --lib` (kernel_env's lib half, `native_subject`, `islands`), `-factions --lib` (131 with the two Yin tests), `ti4-rule-bot-typed --lib` | 466, 72, 131, 1 passed |
| bridge `--test kernel_env --test play_facts` | 1 + 2 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed (after B8, B9) |
| `ti4-engine-core --test adapter --test state_size` | ok (the `Aftermath.produced` field changes no size budget) |
| `ti4-engine-core-tests` with the D4 flags of `tools/engine_core/d4/run_core_tests.sh` (lockstep profile, debug assertions and overflow checks on for the three core crates) | 130 test binaries, 1540 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 480 files, 0 violations; 482 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (core crates 0 warnings; engine 41 warnings, baseline 41) |

Runs on the way (all fixed above): chain 1 (at the merge plus the hand-over): `content_ids_resolve` (B8), the Yin Devotion test (B6; the dual run showed it was the only failure of 5019),
two of the new core tests (they read the typed events from the ledger; the arm hands them out in `Out.emitted`), `w15_scramble_hosts` (B7), clippy (B10). Chain 2: B8's first
fix was wrong (`#![cfg(test)]` does not exempt a file from that lint; the lint skips only `*_tests/` directories and `fixtures.rs`). Chain 3: the delivery inventory (B9), which no earlier
run had reached.

Mutation checks of the new lines (`chain::producing`, `advance_inner`'s production branch, `Aftermath::produced`) were NOT run (heavy). Equivalence of the hand-over is
established by the three lockstep tests against the string `Game` (they fail if the native chain asks, announces or leaves the position differently) and the three core tests.

`tools/engine_core/playthrough_quick.sh` was NOT run: `playthrough --quick` runs the string `Game` under `all=dual` and `--native` is still only printed (`docs/KP_RESUME.md`
and `docs/D2_LF2_RESUME.md`), so no game reaches the native arms of the aftermath family; its table would repeat the dual's counted bails of the lanes and prove nothing about this merge.

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond
`clippy_core.sh`), `decision_replay` and `reaction_corpus` (the other two engine test binaries), mutation batches, `TI4_CORE_STRICT=1`, Playwright. Not pushed.

## 8. Remaining gates, consolidated

Dropped by decision (not open): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel (Alarum, Data Skimmer), Proxima Targeting VI, deck reserve and turn redo,
structured diplomacy. Thunder's Edge: Heart of Ixth, Saar Deorbit Barrage, Arborec Psychospore, the Yin breakthrough and the Ascendant, the Deepgloom Executable, L1Z1X Fealty Uplink, the
Fracture neutral force and the first claim of a Fracture planet, Coexistence, Slumberstate, the TE auto-factories token. Items 1 to 11 of `docs/INTEGRATION9.md` section 6 stand unless
named below.

Open, product-reachable, in the order I would do them. "Silent" means the native path differs from the string WITHOUT a counted bail: those come first, because the zero-bail rule cannot see them.

1. **SILENT: abilities the string registers and the core registry lacks** (M6 open item 2; the live `Env` registry is the core table). Found by the lockstep sweeps: the **Saar agent**
   (`leader:saar:saaragent:SYSTEM_ACTIVATED:after`: the string asks, the core does not) and **Nomad's Mercer** (`...nomadagentmercer:TACTICAL_ACTION_ENDED:after`: the string asks three more
   questions and exhausts the agent after the production step; the native tail does emit `TACTICAL_ACTION_ENDED`). Audit the rest per faction (`ghost.rs` has about 15, Aether Passage,
   `technology:as`) with a registry comparison, and make a string-registered ability without a core twin a counted bail of the live path (`Unknown`). Also silent: the Naalu Matriarch's
   `temporary_ground_force` and the generic may-sustain prohibitions (M3: the live kernel has no provider), and the `previews` of a played card's reaction question (M2, lane A2's display fact).
   Temporal Command Suite itself and Mercer are the Nomad rows of `INTEGRATION9` section 6 item 4.
2. **Tactical activation hooks that ask** (M6 open item 1): Spatial Conduit Cylinders, Scanlink Drone Network (needs the exploration window: now native), Nullification Field and Starlancer
   (end the turn), Minister of Peace, the Dominus Orb (`explore_windows::DominusOrb` exists), Mahact; a **Titans seat** bails `TacticalGap::ActivationHook` at the first activation (seen in the
   dock sweep). Then: ship routes (`Opt::Move.route`, rules version 29+), a space cannon hit that needs a question (`CannonHits`), the tail's Crown of Emphidia / Dark Energy Tap explorations (the
   exploration window is native now), Ssruu-borrowed agents (`SsruuLends`), Nova Seed (the per-step map replay).
3. **Turn flow** (lane B): `CommanderUnlock` (the `commander_unlocked` hook slots of 12 factions) and `leader_action` (17 hero / agent windows) of lane F2 (running), `ChaosMapping` (**unblocked by
   this merge**: `ProduceOne` is resumable data since M2, `OneLink`), `MenuSources` (the closing menu's free options), `AskArm(n)` for every question source but the draft, the starting technology and
   the closing menu (the action menu, the secondary, ...: `ask_ask` / `advance_ask`), `ImperialArbiter`, `QuantumDatahub`, `MapEdits`, `Propagation` (Nekro); `RELIC_GAINED` and
   `BREAKTHROUGH_GAINED` carry the player only.
4. **Production** (M2): the callers other than the aftermath: the Warfare secondary, Construction and Leadership composers (lane E) still run `Spawn::Home` (duplicating the start and end effects
   without `UNITS_PRODUCED`), the leader windows (`ArborecHero`, Muaat) keep `Spawn::Produce` with the adapter's `finish_uses`; they can open `UseOpen::Home` / `UseOpen::Ability` as sub-windows.
   The buildable units and the fleet limit are read when the use starts (a reaction that changes them mid-use is not followed). Exploration Probe still has its own copy of the frontier arms
   (`Fx::Probe`: plan in `docs/M2_RESUME.md`).
5. **Invasion** (M3, counted `Unported`): Dacxive Animators, The Thundarian watching, hits added to a roll, X-89 in the round, the Jol-Nar commander's reroll of the bombardment dice
   (`INTEGRATION9` item 7 for the dual; the live window has no walk for it either), Integrated Economy (dropped, kept as a bail).
6. **Aftermath and combat** (M5): the Spinner (a placement over planets AND ship spaces after `UNITS_PRODUCED`; the production step is native now, so it is reachable), Duranium Armor repair
   queues and a retreat the core does not move (`CombatUnported`), `RollEndsFight`; a Devotion hit plus a retreat of the same round is untested (`round_end_produced` is not kept between two calls);
   the reroll questions (Jol-Nar commander, Munitions Reserves, Hacan flagship, Crowns) are in no live lockstep; `award_tail` for a leaked secret scored as a public objective; the **Yin hero's scoring
   window** (`INTEGRATION9` item 1) is **unblocked**: `EventScoring` exists as a window the hero can open. Van Hauge is native in the live machine; the dual's composed machine still follows the string
   for it.
7. **Action cards played inside another window** (`INTEGRATION9` item 3 and `docs/W_L15_RESUME.md`): Upgrade, Decoy Operation, Harness Energy and Experimental Battlestation have a host now
   (the native activation and movement steps of M6): flip `Fx::nested` per card where the facts are in `SeatPlayMore` / `SharedPlay`; Infiltrate, Bribery, Lie in Wait and Crash Landing wait for
   the control, vote and transaction hosts. The bombardment reroll window and the cannon offense are gated for the Jol-Nar commander and the Heart of Ixth.
8. **Oracle wiring**: `NativeSubject::open_arm` has no line for EventScoring, Tactical and Aftermath (section 5); the observed sub-windows of D1 would have to be typed first for the last two.
9. **No game plays natively end to end**: `playthrough --native` is only printed; the arms are proven by lockstep tests against the string `Game`, one arm family at a time. The wrapper of D2-8
   (`live_reconcile`, `live_step`, the `Asker` that delivers with views: the two `VIEWLESS_ASKS` sites of B9) is what makes the zero-bail playthrough possible, and it has not been written.

Heavy gates still to run once a zero-bail playthrough is possible (commands in `docs/INTEGRATION7.md` 10.4 and `tools/engine_core/end_of_d1.sh`): golden and wide dual sweeps, `board_equivalence`,
`core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census` (and the post-fix numbers of `docs/LENT_CENSUS.md`, which lanes 10 to 15, M2, M3, M5, M6 and B did not re-count), `soak`, the D4
gate steps, the mutation ratchet (`mutation_floor.json`) and the mutation batches named in the resume notes (M2: `production/mod.rs`; M5: `chain.rs` has about 38 unclassified survivors against the core tests
alone; B: see `docs/KP_RESUME.md`), workspace clippy, `TI4_CORE_STRICT=1` on the golden games.

## 9. Tip

`git log -1 core-integration-10` (the last commit is this document's). The private target dir and the scratch worktrees were deleted.
