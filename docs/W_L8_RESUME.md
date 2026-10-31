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

New shared pieces: `Spawn::Seeing`, `kernel::children::SeeingLimits`, `Report::Removed`, `kernel::fx::capture_from_board`, `rules::hero_windows`, `hero_dual.rs`.
`nekrohero`, `yssarilhero` and `cabalhero` leave `leaders::string_only_leader`.
