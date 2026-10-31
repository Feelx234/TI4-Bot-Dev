# Lent bodies, lane 9 (branch `core-w-l9`, from `core-integration-8`, merged with its newer tip 0271ca64)

Task: the remaining lent hero gates as core windows. Pure refactor: RULES_VERSION, serde output and fixtures unchanged. No heavy work
(no golden/wide dual, bench, soak, ratchet, workspace clippy, census) while the end-of-D1 run is in progress.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!` + lockstep test + core-crate tests)

| piece | core | adapter / call site | label |
|---|---|---|---|
| Empyrean hero (Conservator Procyon) | `rules::hero_windows::EmpyreanHero`: places the tokens, explores each token under a ship (log row, holdings, every frontier card arm, children for the Merchant Station / hand limit / Lost Crew), undoes the whole run when the deck runs out (snapshot of the state, after the questions already asked) | `hero_dual::empyrean_hero` (+ `action_cards_view::close_frontier_row`), `factions::empyrean::hero_dual` | `factions::empyrean::hero` |
| Warfare secondary inside the Naaz hero | `Spawn::Home` -> `rules::production::home::HomeProduction` (use counter, Hacan hero offer, discount reset, production window `ProductionWindow::use_of`, end-of-use: value swap, Auto-Factories, Self-Assembly offer through `AreaLinks::spawn_extra`) | `hero_dual::home_of` / `leaders_core::home_facts`, UNITS_PRODUCED announced by `hero_dual::announce_use` | `factions::naaz::hero` |
| Imperial primary inside the Winnu hero | `Spawn::Imperial` (the existing `ImperialPrimary`), `Report::Scored`, tail (`award_tail`, winner) in `hero_dual::winnu_hero` | `strategy_cards_core::imperial_facts` | `factions::winnu::hero` |
| Mahact hero (Airo Shir Aur) | `rules::combat::fight::FightToEnd` (the settle-machine loop as one child window), `rules::combat::mahact_hero::MahactHero` (pair, move, `Spawn::Seeing`, mark and active system, fight, feats) | `dual_window::mahact_hero` (Env read from `CombatWindow::opening_args` on a copy of the position with the fleet moved), `factions::mahact_units::hero_dual` | `factions::mahact::hero` |
| Yin hero (Quantum Dissemination) | `rules::invasion::fight::CommittedFight` (BeginGroundRound, GroundRoll, GroundRollTail, GroundRoundHits in turn, GROUND_COMBAT_STARTED/ENDED, feats), `rules::invasion::yin_hero::YinHero` (commitments, landing, control) | `dual_window::yin_hero` (+ `yin_unported`), `factions::yin::hero_dual` | `factions::yin::hero` |

`leaders::string_only_leader` is empty: every action leader of the lobby factions has a port.

## Found on the way (each with a test that fails on the old code)
* `ti4_engine_core_factions::cd_registry::yin::timing::roster` named the first unit of the enum (the Absol space form of the Naaz Eidolon,
  the base Iconoclast for a Thunder's Edge Naalu): Self-Assembly Routines placed the wrong mech form for a Naaz seat. Test
  `technology_core::roster_tests::the_faction_crate_roster_names_the_unit_the_content_names`.
* `System::name_rank` is the display name, not the id text: the Empyrean window takes its token order from the adapter (lockstep difference
  `exploration_log[i].system` found by the first run).
* A Merchant Station's replenish at a table with Cabal / Trade Agreement is now the core's (`fx::replenish_announced`).

## Remaining bails (counted, each with a test)
Dynamic nested activity (a reaction window around an event of the window: Indoctrination, the Yin agent, action cards in a combat) is the
reaction-table area, not a window: every window of the dual harness bails on it (`outside_why` / `nested_check`).
* Empyrean: a command token while a Mahact fleet pool holds tokens of the seat (the reinforcements preview); a Lost Crew where a faction hook
  changes the draw (Yssaril); a replenish at a table with two seats of one faction.
* Home production (Naaz hero): a Warfare secondary performed after a Leadership / Technology / Construction secondary (the facts of the
  production were read before it; they need the string's `buildable_for` / fleet limit / capacity on the new position); the Prophecy of Ixth in play;
  a swapped planet value; a Keleres / Deepwrought seat, Last Bastion / Crimson (dropped).
* Imperial primary: a Keleres seat (dropped); a point past the victory target; more than sixteen scoreable objectives; ocean cards (dropped).
* Mahact: dice the core cannot reproduce (test faces, a stream restore); everything `Machine::bailed` names (a Letnev / Mentak round opening,
  Duranium Armor, Devotion, a retreat the core does not move); the scoring window of a barrage feat is consumed as `resolve_resolving` does.
* Yin: The Thundarian watching, X-89, Dacxive Animators, Proxima Targeting VI, Alarum (Ral Nel, dropped), hits added to a roll, a Letnev seat
  (Dunlain Reaper facts), a neutral side, a coexisting planet, the Minister of Exploration / Crown of Emphidia / L1Z1X Fealty Uplink on the
  control gain, and a recording in which the string opened a scoring window after a won combat (objective scoring is the string's).

## Checks (cheap only)
See the final report of the lane; commands: `cargo test -p ti4-engine --lib` (also `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`),
`cargo test -p ti4-engine-core-tests --test hero_windows --test production_home --test combat_fight --test invasion_yin`,
`tools/engine_core/check_layers.py core`, `tools/engine_core/lint_core.sh`.
Not run: golden / wide dual, census, bench, soak, ratchet, workspace clippy.

## Mutation checks
Batch scripts as in lane 8 (`/root/deploytmp/core-w-l8/runmut.py`, single-file mutants on a committed snapshot in a scratch worktree).
Files to mutate: `rules/hero_windows.rs` (EmpyreanHero), `rules/production/home.rs`, `rules/combat/fight.rs`, `rules/combat/mahact_hero.rs`,
`rules/invasion/fight.rs`, `rules/invasion/yin_hero.rs`. Status: see the final report.
