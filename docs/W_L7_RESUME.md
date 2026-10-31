# Lent bodies, lane 7 (branch `core-w-l7`, from `core-integration-7` @ 3b97d529)

Task: port the counted string gates lane 6 left as production-window users (Arborec hero, Muaat agent, the Muaat component actions, the single
units of Chaos Mapping / Freelancers) as core windows; then the heroes where they compose from existing windows. Pure refactor:
RULES_VERSION, serde output and fixtures unchanged. No heavy work (no gate, verify_package, golden/wide dual, bench, soak, mutation ratchet,
workspace clippy, census re-run) before the end of D1.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!` + lockstep test + core-crate tests)

| piece | core | adapter / call site | label |
|---|---|---|---|
| Muaat Star Forge and The Nucleus (with the Ember Colossus placements) | `rules::muaat_component::StarForge` (`Mode::{StarForge, Nucleus}`), `Ask::Promissory(StarForge / EmberColossus)` | `faction_dual::star_forge`, `factions::muaat::forge_window` | `factions::muaat::star_forge`, `factions::muaat::nucleus` |
| Muaat The Inferno | `muaat_component::inferno` | `faction_dual::inferno`, `factions::muaat::inferno` | `factions::muaat::inferno` |
| Fires of the Gashlai (Muaat promissory note) | `muaat_component::fires_of_the_gashlai` | `faction_dual::fires_of_the_gashlai`, `factions::muaat::fires` | `factions::muaat::fires` |
| Arborec hero (Letani Miasmiala) | `rules::leader_production::ArborecHero`, child `Spawn::Produce` | `leaders_core::arborec_hero`, `factions::arborec::hero_use` | `factions::arborec::hero` |
| Muaat agent (Umbat) | `leader_production::Umbat` (`LAsk::UmbatPlayer/UmbatSystem`), child `Spawn::Produce` | `leaders_core::muaat_agent`, `factions::muaat::umbat` | `factions::muaat::agent` |
| One unit outside a use of PRODUCTION: Chaos Mapping, Freelancers (W13 `frln*`), legacy Minister of Industry | `rules::production::one::ProduceOne` (`PAsk::One/OnePlace`, `POpt::One`) | `production_window_dual::produce_one`, `production::produce_one_from` (string body `produce_one_text`) | `production::produce_one` |
| Chaos Mapping's choice of the system at the start of a turn | `technology_windows::ChaosMapping`, third stage of `TurnStart` (`with_chaos_mapping`), child `Spawn::ProduceOne` | `technology_dual::start_turn` | `technology::start_turn` |

Shared pieces: `kernel::fx::{max_fit, box_has, place_counted, spend_strategy_token_staged}` (the string `max_fit` / `place_units_counted` over the
fleet supply and the capacity, with the faction rule of `effect_placement_forbidden` read through the kernel), `kernel::children::{ProduceFacts,
OneFacts, Made, Report::Made, Spawn::{Produce, ProduceOne}}`, `ProductionWindow::ability` / `made`, `dual_window::recording()`.
The Muaat family leaves `faction_abilities::string_only_component`; `arborechero` and `muaatagent` leave `leaders::string_only_leader`; the
`Chaos Mapping` bail of `technology::start_turn` is gone.

Found on the way: a production step inside another dual window recorded its question twice under `production=dual`
(`ProductionWindow::resolve_windowed` now leaves the comparison to the outer window). The test `arborec::tests::the_hero_runs_in_lockstep_with_the_core`
runs with both areas dual and fails on the old code (string 6 questions, core 2).

## Still counted gates (documented bails, each counted under its own id, tested)

Action leaders (`leaders::use_leader [gate: <id>: ...]`), each with the reason read in this lane (`leaders::string_only_leader`): nekro hero
(the destroyed ground forces are `hooks_ground` text entries the core's staged-event bridge does not write; candidates are the active deck in
content order), naaz hero and winnu hero (secondary/primary of cards chosen at run time: no `Spawn` for Warfare's home production or Imperial),
empyrean hero (loop of `explore_frontier` with a rollback of state, dice and stream), cabal hero (a die per ship, capture, `fleet::enforce_seeing`
as a child, whole-state rollback), argent hero (the migration over the movement), yin hero (each chosen planet through the eventful ground-combat
window), yssaril hero (`hooks_cards` reveal marks, the stream's `yssarilhero` domain), mahact hero (the whole space combat).

Faction component actions: Saar Deorbit Barrage and Arborec Psychospore (Thunder's Edge, undecided as ordered).

Counted bails inside the new windows (each with a test): a Nekro seat for The Inferno (Z token) and Fires of the Gashlai (Valefar); Umbat copied
through Ssruu; Hacan's breakthrough, the Prophecy of Ixth, a swapped planet value, the Last Bastion / Crimson Rebellion, a Keleres / Deepwrought
seat and a Naaz mech staged in a use (the `UNITS_PRODUCED` announcement of each use is staged by the adapter and would interleave); a single unit that
stands over the fleet supply or the capacity of its system (the string then asks which units go).

Dropped by decision (not ported): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel.

## Checks

Run (cheap only; private target dir `/root/deploytmp/core-w-l7`, `CARGO_BUILD_JOBS=2`): `cargo test -p ti4-engine-core --lib` (445+), the new
core-test files `leader_production` (13) and `production_one` (13), `cargo test -p ti4-engine --lib` (4852 passed, 0 failed, 4 ignored at
commit b06e3e55), `decision_delivery_inventory` (4 passed after the rename-only edit `produce_one_from` -> `produce_one_text`, two rows),
`tools/engine_core/check_layers.py core` (0 violations), `tools/engine_core/lint_core.sh` (0 hits).

Mutation checks (single file, Haiku runners on a committed snapshot in the scratch worktree `core-w-l7-mut`, scripts `/root/deploytmp/core-w-l7/
runmut.py`, `spec1.py`, `spec2.py`, `spec3.py`):
batch 1 (52 mutants: `muaat_component.rs`, `kernel/fx.rs`, `leader_production.rs`) 44 killed, 8 survived. Closed by new tests: the sort of the
neighbours before the forge, the Nucleus exhausting only its own card (`the_nucleus_declined_leaves_the_planet_card_ready`, an assertion in the
Star Forge test), a faction rule that bars a form (`a_faction_rule_that_bars_the_form_hides_it_and_places_none`), the box cap of `place_counted`
(`units_are_placed_only_as_far_as_the_box_holds`), the facts of the chosen player in a shared system. Equivalent mutants: the `sort_unstable` of the
war-sun / flagship / planet lists (the board iterates in id order already) and the "pick was offered" guard of `Umbat::answer` (the driver
rejects an unoffered pick first). Batch 2 (`production/one.rs`, `ChaosMapping`) and batch 3 (re-run of the six closed survivors): see the report.

## Not run (by the directive)

`verify_package`/gate, golden or wide dual run, census re-run (the gate tables are by reading), bench, soak, mutation ratchet, workspace clippy,
board_equivalence, `TI4_AREAS=all=dual` full suite.

## Merge notes

New core files: `rules/muaat_component.rs`, `rules/leader_production.rs`, `rules/production/one.rs`. Shared types touched: `choice::{promissory,
leaders, production, technology}` (new variants), `render/{promissory, leaders, production, technology}.rs`, `kernel/children.rs`,
`driver/children.rs`, `rules/technology_windows.rs` (`TurnStart` is no longer `PartialEq`; its `Stage` is `(u8, Stage)`), `dual_window.rs`
(`recording`), `production.rs`, `production_window_dual.rs`. Likely conflicts with other lanes: `leaders.rs` (`string_only_leader`),
`faction_abilities.rs` (`string_only_component` and its gate test), `decision_delivery_inventory.rs` (two rename-only rows).
