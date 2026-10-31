# Lent bodies, lane 8 (branch `core-w-l8`, from `core-w-l7` @ fedbbd57)

Task: the last lent bodies, nine heroes (`leaders::string_only_leader`): nekro, naaz, winnu, empyrean, cabal, argent, yin, yssaril, mahact.
Pure refactor: RULES_VERSION, serde output and fixtures unchanged. No heavy work (no gate, verify_package, golden/wide dual, bench, soak, mutation
ratchet, workspace clippy, census re-run) before the end of D1.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!` + lockstep test + core-crate tests)

| piece | core | adapter / call site | label |
|---|---|---|---|
| Nekro hero (UNIT.DSGN.FLAYESH) | `rules::hero_windows::NekroHero` (`LAsk::FlayeshPlanet/FlayeshTech`, `LOpt::Devour/Gain`); the destroyed ground forces are written by the core as the typed `private:#staged:ground:` rows (`kernel::fx_ground::stage_destroyed`, cause `LeaderBare(UNITDSGNFLAYESH)`): the staged-event writer for the `hooks_ground` text entries already existed in the Marks slice codec, lane 7 had not seen it | `hero_dual::nekro_hero` (the facts: planets, goods, technologies of the colour, as the string reads them), `factions::nekro_units::flayesh_dual` | `factions::nekro::flayesh` |
| Yssaril hero (Kyver) | `hero_windows::KyverHero` (shows as `Ask::Cards(OwnHand{Show})` of each player, `LAsk::KyverFate`, takes first, then the random discards from the `yssarilhero` stream via `Sx::die`, the hand limit as a `Spawn::DrawCards{count:0}` child) | `hero_dual::yssaril_hero` (streams lent at `rng_before`, compared after), `factions::yssaril::hero_dual` | `factions::yssaril::hero` |
| Cabal hero (It Feeds on Carrion) | `hero_windows::CabalHero` (a die of the dice stream per ship, `kernel::fx::capture_from_board` (moved from `rules::production::supply`, which re-exports it), the stranded cargo is `Spawn::Seeing` = `tactical::fleet::Enforce` as a child (`SeeingLimits` in `kernel::children`, `Report::Removed`), the cargo gone afterwards is captured in unit order) | `hero_dual::cabal_hero` (the limits per owner and system are `tactical_dual::limits_of`), `factions::cabal::carrion_dual` | `factions::cabal::hero` |

| Naaz hero (Hesh and Prit) | `hero_windows::NaazHero` (`LAsk::NaazCard`, `LOpt::HeroCard`; the cards are asked up to twice, the relic is gained by `kernel::fx_relic::relic_gain`, each secondary is the ability's own window as a child through `kernel::fx_perform::begin`) | `hero_dual::naaz_hero` (same facts and detail completers as Strategize), `factions::naaz::hero_dual` | `factions::naaz::hero` |
| Winnu hero (Mathis Mathinus), choice of the card and its primary | `hero_windows::WinnuHero` (`LAsk::WinnuCard`, a primary as a child through `fx_perform::begin`) | `hero_dual::winnu_hero`, `factions::winnu::use_leader_strategy_primary` (the followers stay the `Game` driver's) | `factions::winnu::hero` |
| Argent hero (Flock Migration) | `hero_windows::ArgentHero` (`LAsk::FlockShip/FlockTo/FlockCargo`; the hold from `query::transit::loadable`, the relocation through `fx_move::{move_ship, take_aboard}` with the string's checks: both ends on the map, the destination enterable, the cargo present and within the capacity, else dropped) | `hero_dual::argent_hero` (the map rules are read on the position: `MovementRules::can_enter`, `galaxy.coord_of`), `factions::argent::migration_dual` | `factions::argent::hero` |

New shared pieces: `Spawn::Seeing`, `kernel::children::SeeingLimits`, `Report::Removed`, `kernel::fx::capture_from_board`, `rules::hero_windows`, `hero_dual.rs`.
`kernel::fx_perform` (the body of `rules::action_cards::perform` lifted to the kernel: `Carried`, `Facts`, `Began`, `begin`, `finish`; Strategize and Overrule call it,
the Warfare secondary / Imperial primary / Thunder's Edge Construction and Warfare primaries stay `Began::Unsupported`), `strategy_cards_core::ready_hints_now` now takes the
exhausted planets from the core both ways (a second secondary in one window sees what the first one paid).
`nekrohero`, `yssarilhero`, `cabalhero`, `naazhero`, `winnuhero` and `argenthero` leave `leaders::string_only_leader`.

## Still counted gates (documented bails, each counted under its own id, tested)

`leaders::use_leader [gate: <id>: ...]` (test `the_action_leaders_that_stay_in_the_string_are_counted_at_the_dispatch`), the reasons as read in this lane:

* `empyreanhero`: the core has the primitives of an exploration (draw, fragment, attachment, wormhole token, the reward windows) but no window that explores
  a frontier token; `exploration::explore_frontier` is `explore_in` (the exploration log row opened and closed with the `Holdings` difference and `row_words`,
  the card's own arm, the `PLANET_EXPLORED` hooks), run once per token under a whole-state rollback of state, dice and stream. A window that placed the tokens
  and bailed when a token holds a ship would compare only the uncommon no-target use, so it was not built.
* `yinhero`: the Game path fights each chosen planet with `InvasionWindow::fight_committed_planet`, the eventful ground combat run in the game's timing resolver
  (rounds, reactions, nested emissions, which the window recorder marks `nested`/`foreign`), then `establish_control` runs the string `finish_control_gain` chain.
* `mahacthero`: the fleet's move and the `Enforce` child exist (`Spawn::Seeing`), but the fight is `combat::resolve_resolving` inside the timing resolver; the core
  combat `Machine` settles one decision at a time for the `Game` combat driver (its `Env` is `core_combat::window_env`) and is not a window that runs a whole combat as a child.

Counted bails inside the new windows (each with a test): Nekro, a galvanized ground force on the devoured planet (the typed staged row has no `|g` tail);
Yssaril, a turn with blank laws and a faction-changed hand limit (the hand-limit child reads the printed rule and the laws); Cabal, none (the limits come from
`tactical_dual::limits_of`, its own bails count under the label); Naaz and Winnu, the Warfare secondary (the home production: `production::resolve_timed` adds the
Hacan hero offer, the Keleres agent window and the use counters around the window), the Imperial primary (scoreable objectives, award tail, winner), the Thunder's
Edge Construction and Warfare primaries, a Politics draw a hook changes, a research a faction waiver opens, a Leadership payment the production kernel does not
follow, a table where a replenishment causes events (Cabal, Firmament siphon, Trade Agreement), and a Mahact-held command token (`fx_perform::begin`; tests
`the_warfare_secondary_is_not_carried_and_makes_the_window_unsupported`, `the_imperial_primary_is_not_carried_and_no_cards_means_no_question`).
Not mine: Saar Deorbit Barrage and Arborec Psychospore (Thunder's Edge, left as documented bails as ordered).

Dropped by decision (not ported): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel.

## Checks

Run (cheap only; private target dir `/root/deploytmp/core-w-l8`, `CARGO_BUILD_JOBS=2`): `cargo test -p ti4-engine-core --lib` (451), `cargo test -p ti4-engine-core-tests --tests`
(1165 passed; the new file `tests/hero_windows.rs` has 32), `cargo test -p ti4-engine --lib` (4860 passed, 0 failed, 4 ignored), `decision_delivery_inventory` (4 passed
after the rename-only edit `use_leader_strategy_primary` -> `hero_primary_string`, two rows), `tools/engine_core/check_layers.py core` (0 violations), `tools/engine_core/lint_core.sh` (0 hits).
Lockstep tests (`dual_window!`, `Area::LeadersAbilities=dual`, zero differences and the window compared each time): `nekro_units::tests::the_hero_runs_in_lockstep_with_the_core`
(+ the galvanized bail), `yssaril::tests::the_hero_runs_in_lockstep_with_the_core` (+ the blank-laws bail), `cabal::tests::the_hero_runs_in_lockstep_with_the_core` (with the real dice stream),
`naaz::tests::the_hero_runs_in_lockstep_with_the_core` (every card of the board), `winnu::tests::the_hero_runs_in_lockstep_with_the_core`, `argent::tests::the_hero_runs_in_lockstep_with_the_core`.

Mutation checks (single file, Haiku runners on a committed snapshot in the scratch worktree `core-w-l8-mut`, scripts `/root/deploytmp/core-w-l8/runmut.py`, `spec1.py`, `spec2.py`):
