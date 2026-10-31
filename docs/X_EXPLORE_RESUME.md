# X lane (`core-x-explore`): the planet exploration window, resume note (2026-10-31)

Branch `core-x-explore` = `core-w-l14` (06c7191f, contains lanes 9 and 13) + the merge of `core-m3-invasion` (f7241b96) + the commits below. Never pushed; the live branch
`nightly-fixes-2026-10-26` is untouched. Pure refactor: `RULES_VERSION`, serde output and fixtures are unchanged. No heavy work (no golden/wide dual, bench, soak, ratchet,
workspace clippy, census): a full zero-bail playthrough is not possible yet (see "Left").

## Merge note (`core-m3-invasion` into `core-w-l14`)

Conflicts, all "keep both sides": `hero_dual.rs` (l14 `drive_positioned` + facts with `galaxy` / M3 `window_driver::driver_for`: both, the driver is built with `driver_for`),
`reactions_core.rs` (M3 `set_table_codes` + l14's `Codes::indoctrination`, added to the function), `strategy_cards_core.rs` (l14 `ImperialPrimary::with_price`
+ `facts.scoreable` with M3's `driver_for`), `rules/invasion/{mod,begin}.rs` (both modules `fight` and `forced`; `reaper_offered` takes the `can_pay` the l13 `pay_env` reads),
`rules/production/strategy_cards.rs` (`Construction` keeps l14's `uses/using/produced/unsupported` and M3's `alt_at`), the two docs. Fallout fixed: five `Driver::new` of l9/l13
adapters in `dual_window.rs` / `hero_dual.rs` now use `window_driver::driver_for` (the guard test `no_adapter_builds_a_driver_without_the_constructor`), `StructureFacts::alt` in two core tests,
and the engine's `turn_write::TURN` footprint carries `Pending` (M3's `end_turn_scratch` forgets `failed_component_actions`; the dual test
`copied_naaz_nested_exploration_error_rolls_back_and_same_game_retries` panicked "slice not loaded" under `all=dual` before).

## What is native

### One exploration window (`rules::explore_windows::Exploring`, state `state::explore_run::ExploreRun`)

`Exploring` is the string `explore_in` with the deck choice of its callers (`exploration::choose_deck`), for a planet or a frontier token: the deck (asked among the traits now:
printed traits in the scope, then the traits attachments give; `EDeck::Choose`, or given: `EDeck::Fixed`), the draw (an empty deck: nothing explored, no row, `empty()`), Naaz
Distant Suns (the second card is drawn, which to resolve is asked, the row opens after), the log row opened (`fx_explore::open_record`), every card arm, the holdings around the card,
the row closed with the adapter's words, then for a planet the faction hooks (`economy.explored`: Pre-Fab Arcologies) and `PLANET_EXPLORED` (when the string explores with a timing handle).

* Arms: relic fragments; Attach (the research facilities take their `...stat` form on a planet that prints a specialty, the Demilitarized Zone demilitarizes from rules version 34,
  an attachment drawn for a frontier token is `Discarded`); entropic fields / Keleres ships (the pool question from version 12, then the goods, announced); gamma wormhole / relic;
  ion storm (child `IonStorm`, the planet's system); Mirage (the planet's system, the token's system for a frontier from version 27); Enigmatic Device; Dead World; Abandoned Warehouses,
  Merchant Station, Functioning Base, Local Fabricators, Mercenary Outfit (child `Reward`, the unit placed through `fx_strategy::place_for`); Core Mine, Expedition, Volatile Fuel Source
  (child `PayMechOrInfantry`); Derelict Vessel (secret + hand-limit child) and Lost Crew / the Functioning Base card (action card draw child), both children of other areas through
  `Spawn` links; an unknown card is `Unresolved`.
* The window keeps NOTHING outside `ExploreRun` (plain data in the state layer; the children are rebuilt from it with the new `asking(..)` constructors of `ChooseDeck`, `DistantSuns`,
  `IonStorm`, `Reward`, `PayMechOrInfantry`), so a host stores it in its payload. `Exploring::hosted(run)`: a child of another area runs at once when it is started (a draw within the hand limit),
  and one that asks (the discard of a draw over the limit, the hand limit of a secret) is the named bail `EWhy::LinkedAsks`.
* The adapter owns the words: `AdapterFacts::explore_words` (`exploring_dual::words` = the string `row_words` over `RowIn`, minted in the position's text table; the core never sees a text),
  `Codes::explore` (the event texts: the source `exploration`, the deck and card texts of `PLANET_EXPLORED`, `trade_agreement`). A table without them declines (`EWhy::NoTexts`).
* One owner per card: lane 9's Empyrean hero (`EmpyreanHero`) is now only the placing of the tokens and the loop over the systems under a ship; each token is a child
  (`Spawn::ExploreFrontier`, `Report::Explored(ExploreEnd)`); the ~400 lines of card arms, `FrontierFacts`, `FrontierRow` and `action_cards_view::close_frontier_row` are gone. The undo of a run whose
  frontier deck runs out stays the hero's (snapshot).

### The string callers and the dual

`exploration::explore_with` (planet) and `explore_frontier` are `dual_window!` sites (`exploring_dual::{explore_planet, explore_frontier}`; label `exploration::explore` / `exploration::explore_frontier`),
so EVERY caller is covered at once: the invasion's first claim, the Crown of Emphidia and Dark Energy Tap of the tactical tail, the Naaz / Titans / Cabal / legendary / action card callers, the Empyrean hero (nested). The old
per-step dual windows inside `explore_in` (ion storm, rewards, pay, Distant Suns) are string-only under the outer window now (their tests count `exploration::explore`); the deck choice
(`exploration::ask:exploration`) keeps its own window beside it. `invasion::control_gain_exploration` is no longer a gate (`note_gate` removed). Stays with the string (counted bail, label
`exploration::explore [bail: ..]`): Freelancers, Titans' Terragenesis, a replenishment of two seats of one faction, a draw a faction hook changes, a turn redo's reserve, a space battle, diplomacy.

### The invasion island (`IStage::Exploring`)

`IStage::Exploring { planets, index, planet, run: Box<ExploreRun> }` after `ControlGained` of a first claim (`previous.is_none()` and a trait to explore into); `finish_control_gain` returns the run, `advance_control`
and the `FinalizingControl` settle step suspend there, `question` / `resolve` / `settle` of the machine route to `Exploring::hosted`, and the chain goes on with the next planet after it. The bail
`ExplorationOfAFirstClaim` is gone; `Unported::Exploration(EWhy)` names what the window does not carry. `env_for` (live) installs the event texts and the words.

### The other control-gain bails and the defense (M3_RESUME "Counted bails")

* Minister of Exploration (staged trade good, source `InvasionCodes::minister`) and the Crown of Emphidia (`fx::steal_throne_card`) in `establish_control`: native (lane 13's Yin hero already was).
* A ship-form unit landing (Naaz Eidolon): `CommitGround` and `LandOne` flip it to its ground form (`fx_strategy::flip_form`) as `invasion::land_unit` does; the `unsupported` marks are gone.
* Space cannon defense: an entropic scar and L4 Disruptors (no gun fires; the commitment is still announced) and an attachment that gives SPACE CANNON (Titans' Geoform: rolled for the controller after the guns,
  not a die of the step, its hits count) are in `SpaceCannonDefense`; `dual_window::defense_unported` lost the three reasons.
* Assimilate without a roster: the live `Env` carries the roster (`env_for`), so only a hand-built table without one still names `AssimilateRoster`.

## Tests (cheap)

* core-tests `exploring` (19): deck choice, every arm family, the Demilitarized Zone by version, announce / quiet, a hosted run stored and resumed, a hosted child that does not ask runs at once and one that asks is named, Freelancers / Terragenesis / no texts named.
  `hero_windows` (67, the Empyrean tests now run through the child), `invasion_composer` (7: a first claim explores natively; a table with no texts is `Exploration(NoTexts)`), `invasion_extra` (the flip), `invasion_windows` (scar, L4, attachment cannon).
* engine `exploring_dual::tests`: EVERY card of every deck explored from a planet (hacan / naaz / titans seats, first and last option) and as a frontier token, in lockstep under the dual, zero differences, the only bails Freelancers and the Titans.
  `invasion_live::tests` (11, with `TI4_BAIL_IS_FAILURE` semantics): `a_first_claim_explores_natively_in_lockstep_with_the_string_window` (36 seeds x 12 invader factions x the three traits, questions / log / decks / attachments / tokens /
  holdings equal, zero bail), the Minister + Crown, the Eidolon, L4 + Geoform; `a_first_claim_the_window_does_not_carry_is_a_bail_...` (Titans).
* Regression tests that fail on the pre-fix code: the first-claim lockstep failed at seed 35 (a Functioning Base draw lost: a child started by an answer was dropped when the host rebuilt the window; `link` now runs it at once),
  the Minister / Crown / flip / defense tests were counted bails before; the hero tests failed to compile against the old `FrontierFacts`.

## Left (precise reasons)

* **Freelancers** (a unit produced with influence) and **Titans' Terragenesis** ("after you explore a planet") are named bails (`EWhy::Freelancers`, `EWhy::Terragenesis`). Freelancers is `Spawn::ProduceOne` with `Meta::influence_pays_for_units`; its state
  (`OStage`, `OneFacts`) is plain data but lives above the state layer, so the hosted run cannot keep it between two questions (move the two types to the state layer, add `ProduceOne::resume`, compute `OneFacts` in the core: `production::home::{buildable_for, fleet_limit}` exist).
  Terragenesis is not a rule of the core table (`cd_registry/titans/offer.rs` is empty: B7); the sleeper mark exists (`MarkKey::TitansSleeper`). Both stop a zero-bail game with a Titans seat / a Freelancers draw (3 of the 51 non-fragment cards).
* **A draw over the hand limit / a third secret** inside an exploration (the discard question of `DrawCards`, `HandLimit`): `EWhy::LinkedAsks` in the hosted run (fine in the dual, where the window is held in memory). Needs `DrawCards` / `HandLimit` to be resumable from data.
* **The Jol-Nar commander's reroll of the bombardment dice** (`Unported::JolNarCommanderReroll`): the per-die question walk is the barrage's (`rules::combat::barrage`, `Facts::commander`); the bombardment needs the same walk in `IBombard` (a cursor and the picks) and the redraw of the staged set. Not done.
* **Exploration Probe** (`rules::action_cards::effects`, `Fx::Probe`) still has its own copy of the frontier card arms (lane 11); it can become a `Spawn::ExploreFrontier` child like the Empyrean hero (the play table needs `exploring_dual::table`).
* `Integrated Economy`, Slumberstate, Dacxive / Thundarian, X-89 in the round (M3_RESUME) are unchanged.
* The census (`docs/LENT_CENSUS.md`, `tools/engine_core/lent_sites.tsv`) is not re-counted (heavy); the line numbers of the tsv are stale as always.

## Checks (private target `/root/deploytmp/core-x`, `CARGO_BUILD_JOBS=2`, test binaries deleted afterwards)

* `cargo test --no-fail-fast -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-tests`: green (one stale test of W14 fixed: `promissory_windows::mageon_implants_takes_a_single_card_unasked_and_a_hand_over_the_limit_asks_which_card_goes` seated a Yssaril
  seat, whose Crafty lifts the limit `DrawCards` reads from the kernel since W14; the limited case now seats another faction).
* `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test --no-fail-fast -p ti4-engine --lib` (the whole lib, once at the end): 4932 passed, 4 failed, 4 ignored; the 4 were all mine and are fixed (re-run: `invasion_live`, `strategy_cards_core`, `yssarilbt`, `exploring`: 45 passed
  under `all=dual`): `a_first_claim_explores_natively_in_lockstep_with_the_string_window` (the pool question of a command token inside an invasion lacked `invasion_seq`: `Exploring::ask_of` now names the open invasion on every question, children of other areas included;
  core test `a_question_asked_inside_an_invasion_carries_its_sequence_number`), and three Politics / Deepgloom dual tests (a slip of my merge resolution in `strategy_cards_core::drive_window_on`: it shadowed the `kernel` argument with `Kernel::plain()`, so the
  Politics draw lost the Scheming / Crafty hooks; restored). The plain-areas lib run was started and stopped (the machine was loaded; the `all=dual` run subsumes it).
* `tools/engine_core/check_layers.py core`: 453 files, 0 violations (the first version imported `rules::explore` from `explore_windows` by path); `tools/engine_core/lint_core.sh`: 0 hits (the first version put `String` in `kernel::fx_explore`:
  the words are minted by the adapter, the core closes the row with codes).
* Mutation: see below.

Not run: golden / wide dual, benches, soak, ratchet, workspace clippy, census (a full zero-bail playthrough is not possible yet).
