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
