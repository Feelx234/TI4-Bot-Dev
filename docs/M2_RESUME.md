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
