# D2 lane M2 (`core-m2-production`): the production step island, resume note (2026-10-31)

Branch `core-m2-production`, cut from `core-x-explore` (8df15b0a) and merged with `core-d2-f-live2` (bfbab082: NativeSubject, play facts, `playthrough --quick`, lane W11;
conflicts kept both sides: `Unknown::Unported` stays, the `ReflectiveShielding` gate went with W11, the settle run uses `tables_of_sides`, the defense gates of lane X stay).
Never pushed; the live branch is untouched. Read `docs/D2_LF_RESUME.md` / `docs/D2_LF2_RESUME.md` (the live-loop contract) and `docs/M3_RESUME.md` first.
Pure refactor: `RULES_VERSION`, serde output and fixtures are unchanged; no heavy work was run (no zero-bail playthrough is possible yet).

## What is native

### One use of PRODUCTION as a held window: `driver::windows::production` (`ProductionMachine`, payload `state::ProductionUse`, `Arm::Production`)

| piece | where |
|---|---|
| payload (replaces the stub `{actor, data}`) | `state/production.rs`: `ProductionUse {who, system, mode: UseMode, phase: UsePhase, window: ProductionWindow, answered}`; the plain data of the production window (`ProductionWindow`, `PStage`) moved here, `rules::production::window` re-exports them and keeps the `impl`s |
| the machine | `driver/windows/production/mod.rs`: `Window` + `Held` (`KIND = Production`); `live.rs`: `open_production(core, UseOpen)`, `advance_production`, `ask_production` (the generic `driver::live::advance`: nothing pauses a use) |
| the arm | `Arm::Production` (not chosen by `step_arm`: the caller that hands a use over runs it by name, as the aftermath runs the invasion), registered in `driver/live.rs` (`live_arm`, `ask_arm_fn`), `NativeArms::ALL` 21 bits |
| facts | `home_facts` (existing) at the start of the use; `ProductionWindow::refresh` (new, `rules/production/window.rs`) is the string `refresh`; `Codes::production` (`ability`, `production`, `units`) filled by `production_live::fill_codes` from `reactions_core::set_table_codes` |
| display facts | none to add: `live::render_asking` now renders with the position (`Typed::new_at`, `live::kernel::static_of`), which the production options read (`Render::position`) |
| events | `UNITS_PRODUCED` is typed (`List::Produced` in `state/event.rs`, rendered by `dual_window::event_of`); `PRODUCTION_USED` as before |

The three openers (`UseOpen`), in the order of the string:

* `Tactical` = `enter_production`: capacity 0 ends it at once (no number, no events); else `production_seq += 1` BEFORE the Hacan hero is offered (T-I2-2), the hero offer
  (`HeroOffer`), Sarween Tools / AI Development Algorithm (`ProductionUsed`), `PRODUCTION_USED`, `refresh`.
* `Home` = `production::resolve_timed` (the Warfare secondary): the discount an earlier action left is dropped from version 26, the hero is offered when unlocked and the
  capacity is positive (the number goes up first), `refresh` only when the hero was used; no `PRODUCTION_USED` and no `production_used` (as the string).
* `Ability` = `produce_by_ability` (`Spawn::Produce` callers): the caller's facts are the window; no start effects.

The end of the use (`finish_use`, T-I2-1: after an answer, in the step that finds the window done; a use that asked nothing has none): the planet-value swap goes (planet and the
`ProductionValueSwapSeq` mark), the Prophecy of Ixth is discarded unless two fighters were made, `UNITS_PRODUCED` (`source` ability/production, `count`, `units[{unit_type, place}]`),
Self-Assembly Routines (not for an ability's units, version 13+, owned; the table's link). The Thunder's Edge Auto-Factories token is NOT ported (agreed scope).

All questions of the start and end effects (hero, AIDA, Self-Assembly, reactions to the events) go through THE TAPE (`Sx::ask`): the driver rolls the step back and replays it with the
answers, so the payload never holds a half-run child; only the production window's own questions (build, pay, place) are stage questions. `Pending::nested` carries the rest.

### ProduceOne's plain state in the state layer; Freelancers in a hosted exploration

`state::{OneRun, OStage, OneFacts}` (plain data; `kernel::children::OneFacts` is a re-export), `ProduceOne::{resume, run}`. A new link `AreaLinks::one: OneLink {facts, step, ask, answer}`
(`kernel/children.rs`, filled by `driver::children::register` / `one_link()`) lets a rule area that may not import the production area keep the unit in its own payload:
`ExploreRun::one` + `EStage::Producing`; `Exploring::freelancers` computes the facts in the core (`home::buildable_for`, `home::fleet_limit`), sets `influence_pays_for_units` for the
unit and clears it when the unit ends (an over-supply unit stays the named gap `EWhy::Freelancers`). A host rebuilds the window with `Exploring::hosted(run).with_one(one_link())`
(invasion `ops.rs`/`fight.rs` do). `ExploreCodes::freelancers` is the printed name (`exploring_dual::codes`).

### Titans' Terragenesis as a rule of the core table

`cd_registry/titans/offer.rs`: `TERRAGENESIS = Slot(17001)` on `PlanetExplored`/After, optional, `Ask::Reaction(RAsk::Terragenesis{planet})`, `Opt::Sleeper{from}`,
`Subtype::Terragenesis` (`choice/reaction.rs`, rendered in `render/reaction.rs`, named in `reactions_core::NAMED`, `Codes::terragenesis`). The marks are the string's
(`MarkKey::TitansSleeper`). The gate in `Exploring::start` is gone. The DUAL driver of `exploring_dual::run_window` has no registry, so a Titans planet exploration with a timing handle
stays a counted bail THERE (`Terragenesis` text); the live `Env` registry has the rule.

### Hand-limit discard inside a hosted exploration

`Exploring::link_on_tape`: a linked child that asks (the discard of a draw over the hand limit, the secret objective's hand limit) puts its questions to the tape; `EWhy::LinkedAsks` is
only left for a child that hands back a `Next`.

## Not done (precise)

* **Exploration Probe** (`rules::action_cards::effects`, `Fx::Probe`, ~110 lines of `maybe_explore` + `merchant_station` + `ProbeRow`/`Explored` + the adapter's `action_cards_view::close_row`
  and `announce::explored`) still has its own copy of the frontier card arms. Plan: after the token is picked, run `Spawn::ExploreFrontier{who, system}` as a child of the play window
  (`ErasedChild`), read `Report::Explored`, and let the exploration area close the log row with `explore_words` (the plays table needs `exploring_dual::table`'s links, words and
  `ExploreCodes`; `live::env::env_for` already sets them); delete the arms, `ProbeRow`, `close_row`; the tests `action_card_effects3` and the `bail_exploration_probe_*` series move to the child.
* **NativeSubject** cannot wire `Arm::Production`: it chooses the arm by `step_arm`, which never returns it (like `Arm::Invasion`); it is reached through the aftermath arm (lane M5:
  `chain::producing` opens `UseOpen::Tactical` and `advance_aftermath` hands its steps over while `Window::Production` is the top sub-window, then writes `PRODUCTION_RESOLVED`).
  `d1_action::sub_of` gives the dual the observed payload (`ProductionUse::observed`).
* The callers other than the aftermath: the Warfare secondary / Construction / Leadership composers (lane E) still run `HomeProduction` as a child (`Spawn::Home`); it duplicates the start
  and end effects without `UNITS_PRODUCED` (the dual adapter announces it). They can open `UseOpen::Home` as a sub-window when lane E hands over; the leader windows (`ArborecHero`,
  Muaat) keep `Spawn::Produce` with the adapter's `finish_uses`.
* The `buildable` units and the fleet limit of the window are read when the use starts (the string reads them at every question); a reaction that changes them mid-use is not followed.
* Keleres' agent window, Agency Supply Network, Deepwrought (dropped by decision).

## Findings fixed on the way (each guarded by a test that fails on the old code)

* the tape runner dropped the finished `HeroOffer` (`Child` hands the window back on `Done`), so a taken hero left a HOME use at full price: core test `a_home_use_with_the_hero_taken_makes_every_unit_free` (fails on the old runner, checked) and the lockstep `the_hacan_hero_makes_the_use_free_and_is_purged_in_lockstep` (Home).
* a hosted Freelancers unit was asked its stale payment after the answer (`pay -3 more resources`: the host asks before it steps): `Exploring::answer` now settles the unit at once; guarded by `invasion_live::a_titans_first_claim_explores_natively_with_terragenesis_in_lockstep_with_the_string_window` (seed 9 failed before).
* Freelancers resolves the card only when a unit stands (the string's answer is "was a unit made"): `exploring_freelancers` asserts `how`.
* `Slot(15001)` clashed with the Instinct-Training/other slots (`debug_assert` "registered twice" in the engine table): Terragenesis is `Slot(17001)`.
* `live::render_asking` rendered without a position, so the options of a production question panicked ("a listed option is offered in its position"): it renders with `Typed::new_at` now.
* Finding for lane A2: `live::env::env_for` names the faction units of the FULL source scope (`roster_for(.., FULL)`), a POK game's Naalu mech is `naalu_mech` but the live roster says `naalu_mech_te`; the lockstep overrides the roster with the game's scope. The `previews` of a played card's reaction question (War Machine) are missing from the native context (display fact of the play slot), the lockstep strips them.

## Tests (cheap)

* core-tests: `production_use` (17: modes, start and end effects, journal across a dropped driver, clone mid-window, payload identity, gaps), `titans_terragenesis` (4), `exploring_freelancers` (2), `exploring` (hosted hand-limit discard through the tape, invasion seq on a tape question), existing `production_home`/`production_one`/`invasion_composer`/`hero_windows` unchanged and green.
* engine `game::production_step_tests` (child of `game`, calls the private `AftermathWindow::enter_production`; declared by 3 lines in `game.rs` like `cannon_prelude_tests`): the string `enter_production` + window (`resolve` ends with `finish_use`) / `production::resolve_timed` / `produce_by_ability` against the native `Arm::Production` through `live_step`, compared question by question (player, prompt, options, details, context), events (name and payload) and the whole position, bail = failure: a plain use, every faction of the product in three modes (Tactical, Home, Ability), Sarween Tools, AI Development Algorithm (used / declined), the Hacan hero (both modes, used / declined), Prophecy of Ixth (destroyer / fighters), Self-Assembly Routines (used / declined), War Machine (played / declined), a system that produces nothing. `invasion_live` (Titans first claim with Terragenesis, Freelancers cards now in the decks of the first-claim sweep), `exploring_dual` (Freelancers no longer bails but the over-supply unit; Titans bail only in the dual driver).

## Mutation checks (single file, Haiku runner on the committed snapshot be75e9ba in a scratch worktree, `tools/engine_core/mutants.sh`)

| file | first run | after the survivor tests |
|---|---|---|
| `driver/windows/production/mod.rs` (vs `production_use`) | 38/52 killed, 14 survived | tests added for the hero gate (locked / capacity 0 / counter +1), the Home close (swap cleared), `made()`; the four loop bounds carry `// mutants:skip` (bounds no window reaches). Not re-mutated. The `emit_timing = true` (#8) and `0 -> 1` (#11, window `capacity <= 0`) points are covered by the engine lockstep only |
| `cd_registry/titans/offer.rs` (vs `titans_terragenesis`) | 24/29, 5 survived | ctx assertions added (`optional`, `space_battle`); two defensive guards (`||` of the effect, the placement guard) carry `// mutants:skip`. Not re-mutated |
| `explore_windows.rs` new fns only (vs `exploring`, `exploring_freelancers`) | 6/10, 4 survived | `how` asserted for a declined / made unit, the invasion number on a tape question; the loop bound carries a skip. Not re-mutated |

## Checks (private target `/root/deploytmp/core-m2`, `CARGO_BUILD_JOBS=2`, no incremental)

See the final report of the lane for the numbers of the last run. `tools/engine_core/check_layers.py core` 0 violations, `lint_core.sh` 0 hits.
Not run (standing rule): golden / wide dual, benches, soak, ratchet, census, workspace clippy, `playthrough` (the aftermath arm of lane M5 does not call `Arm::Production` yet, so a game does not reach the island natively).
