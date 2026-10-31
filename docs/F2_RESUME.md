# D2 lane F2 (`core-d2-f2-hooks`): kernel hook slots and the leaders menu sources, resume note (2026-10-31)

Branch `core-d2-f2-hooks`, cut from `core-d2-f-live2` (`bfbab082`). Never pushed; `nightly-fixes-2026-10-26` untouched. Read `docs/D2_LF2_RESUME.md` (the open list this lane closes)
and `docs/D2_LF_RESUME.md` (the recipe) first.

## What is native

| # | Item | Where | Evidence |
|---|---|---|---|
| 1 | The 12 `commander_unlocked` and the 17 `leader_action` hooks (arborec, argent, cabal, empyrean, ghost, mahact, mentak, muaat, naaz, nekro, nomad, saar, titans, winnu, yin, yssaril) | `ti4-engine-core-factions/src/<package>/<faction>/query.rs`, section "leader hooks (package F2)" | `kernel_env` bridge test: `no_hook_slot_is_empty_but_for_the_dropped_factions` (the only gaps `check_kernel_env` prints are the 11 leaders of the dropped factions), `the_hook_slots_agree_with_the_string_on_positions_that_reach_every_condition` |
| 2 | Helper queries in the core | `query/leader_reads.rs` (`space_pieces`, `planet_pieces`, `count_pieces`, `has_entry` = the string board has an entry = `SysFlags::TOUCHED`), `query/reach.rs::assimilators_with_tokens`, `rules/production/probe.rs` (`would_ask`, `ability_facts`, `buildable_for`, `fleet_limit`: whether a production by an ability asks anything, for Letani Miasmiala and Umbat), `cabal/movement.rs::{has_tear, any_tear}` | same tests |
| 3 | Menu sources: strategic action (or lone "pass"), leaders (own + Ssruu copies), technologies (Sling Relay) | `driver/menu_sources.rs` (`strategic`, `leaders`, `technology`, `leader_is_free`, `delivered`), typed `choice::turn::Opt::{Leader, Borrowed, Tech}`, `render/turn.rs` (render and parse), `ti4-engine/src/menu_sources_view.rs` (string position -> core lists, rendered), `menu_dual.rs` builds the menu from them | core unit tests (`driver::menu_sources`), bridge `menu_sources` (lockstep with `strategy::strategic_action_options`, `leaders::component_actions`, `technology::component_actions`, 5 card deals x 21 variants x 18 seats), `menu_dual_tests` under `TI4_AREAS=all=dual` (oracle: the assembled menu, and the closing menu's free flag of a leader, are compared with the string's question) |
| 4 | Initiative order in the live resolver | `driver/live.rs::sync_clock` (one line, textually identical to lane M6b's commit `69eab360`, so the merge is clean; `invasion/live.rs` and `tactical/live.rs` are not in this base: M6b already routes them through `live_common::driver_of`) | `driver::live::tests::a_live_driver_offers_its_windows_in_the_initiative_order_of_the_position` (fails on the pre-fix code: `[Seat(2)]` instead of `[2, 0, 1]`) |

## Test infrastructure added (bridge)

* `check_hooks.rs`: three 6-seat tables seating every faction that owns one of the 29 functions, and ~20 variants that satisfy and miss each condition (12 ground forces / 11, 6 armed units / 5, three rifts, three wormhole systems, a tear, a flock destination, war suns, no money, no ships, empty box, no hand, relic deck gone, geoform, a spent Nomad hero, Nekro assimilator tokens, Ssruu, Sling Relay ...).
* `EnvReport::outcomes` counts what the string answered per method and leader; the test demands both answers for every leader (an answer that is only ever `false` proves nothing). The one exception is `yinhero`: infantry have no plastic limit, so the string answer is always `true` (by construction).
* `check_menu_sources.rs` + `tests/menu_sources.rs`.

## Findings fixed on the way (each with a failing-before test)

* `KernelEnv::strategy::Env::secondary_is_free` answered the hook only and missed Hacan's printed Masters of Trade (Trade card). Found by the new table 3 (Hacan seated); the old template sample did not seat Hacan where the comparison ran. Test: the two new `kernel_env` tests fail on the old code.
* The live resolver carried an empty initiative order (M5's finding): item 4.
* Three registry-count tests of the faction crate (`argent`, `nomad`, `saar`) now count the two new slots.

## Departures from the string (by decision; out-of-scope content or unreachable positions)

* The string `leader_action` hooks read `SourceSet::all()`/`DEFAULT`; the core reads the game's source scope (a unit standing in a system out of the game's sources is a position the game does not reach). The variants therefore avoid out-of-scope systems.
* Thunder's Edge: `xxchahero-te` stays in `delivered` (parity) but `other_action_resolves` is false (KF3 decision).
* `nomadcommander` counts `objectives().scored[seat].secret`; the string counts the scored objectives whose card is a secret (a leaked secret scored as a public one may differ: not reached by any variant).
* `production::probe` copies the readings of `rules/production/home.rs` (`buildable_for`, `fleet_limit`, `armada`), which exists only in the M2 lineage (`core-w-l12`), not in this base. When the lines merge, delete the copy in `probe.rs` and call `home.rs`.

## Not done (with reasons)

* The Faction, Mapped, Relics, Exploration, Action cards, Transactions, Contacts, Payments and Thunder's Edge sources stay lent as ids (`Opt::Lent`). Faction/Mapped are 19 module functions with their own texts and codes (the KM1 "4-8 h" row); transactions belong to the trade lane; contacts/payments are dropped with diplomacy; Thunder's Edge is refused content.
* Dispatch of the chosen leader/tech option (`use_leader` router, KM2) stays `Dispatch::Lend`.

## Hand-over to lane B (`core-d2-c2-turn`)

After merging: delete `COMMANDER_HOOK_MISSING` in `driver/turn/start.rs` and its allowance in the bridge test (`commander_unlocked(..).is_none()` is now never true for a locked commander of those factions); replace `closing::has_free_leader` and the `TurnGap::MenuSources` gate by `menu_sources::{strategic, leaders, technology, leader_is_free}` through `menu::closing_menu` (the free flag of a leader is `leader_is_free`; a Ssruu copy follows its agent). `driver/mod.rs` gets one line (`pub mod menu_sources;`) and `choice/turn.rs` three variants at the end of `Opt`.

## Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/core-f2h` (no incremental; deleted at the end)

* `cargo test -p ti4-engine-core --lib`: 470 passed (+ the probe tests after). `-p ti4-engine-core-factions`: 126 passed (+11 `leader_hook_tests`). `-p ti4-engine-core-bridge` (all tests incl. `kernel_env` 3, `menu_sources` 1): ok. `-p ti4-engine-core-tests`: ok.
* `cargo test -p ti4-engine --lib` default areas: 4921 passed, 4 ignored. `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: the first run failed 2 tests (`faction_effects_tests::d5::nanolink_2...`, `d6::sever_1_and_2...`: the dual menu offered Ral Nel's and Crimson's agents the core has no hook for); fixed (`menu_sources_view::native` returns `None` at a table with a switched-off faction) and both pass, as do `menu_dual`. The whole dual lib was not re-run after that fix.
* `tools/engine_core/check_layers.py core`: 0 violations; `lint_core.sh`: 0 hits.
* Mutation (single-file runs of `tools/engine_core/mutants.sh`, union of core and bridge tests): the hook functions (FN-filtered over the factions crate, bridge `kernel_env`): 180/241, the survivors in OTHER lanes' code (ghost movement, naaz/sardakk/titans/yssaril commanders) are not this lane's; own survivors drove the 11 `leader_hook_tests`, the `probe` tests and the "ssruu, gone agents" variant (not re-run). Left alive and judged equivalent or minor: `yin::leader_action` `> 0` (infantry are uncapped), the cost-halves arithmetic of `arborec::ospha_has_target` (`+4`, `<=`), `umbat_has_target` literals 2 and 4, `ability_facts`' `max(0)`. `probe.rs` 10/23 before its tests, `leader_reads.rs` 7/7, `menu_sources.rs` 18/23 bridge and 4/23 core (the union leaves only the `borrowed` `&&` chain, covered by the added variant), `menu_sources_view.rs` 4/5 (the fifth is equivalent, marked), `render/turn.rs` 2/6 before the parse tests, `driver/live.rs::sync_clock` killed.

Not run (standing rule, no full zero-bail playthrough yet): golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, `playthrough`.
