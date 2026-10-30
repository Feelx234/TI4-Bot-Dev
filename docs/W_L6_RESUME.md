# Lent bodies, lane 6 (branch `core-w-l6`, from `core-w-l5` @ 7085d776)

Task: continue lane 5 — port the remaining counted string gates (component actions first, then the action leaders) to core-owned windows.
Pure refactor: RULES_VERSION, serde output and fixtures unchanged. No heavy work (no gate, verify_package, golden/wide dual, bench, soak,
mutation ratchet, workspace clippy, census re-run) before the end of D1.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!` + lockstep test + core-crate tests)

| piece | core | adapter / call site | label |
|---|---|---|---|
| Nekro C4R place PDS | `promissory_windows::PlaceUnits` + `Then::C4rPds` (structure cap `query::laws::structure_allowed`; `Ask::Promissory(C4rPds)`, subtype `c4r_planet`) | `faction_dual::c4r_place_pds`, `factions::nekro_units::place_pds` | `factions::nekro::c4r_place_pds` |
| Yssaril Mageon Implants | `promissory_windows::MageonImplants` (+ `Ask::Cards(TakeRevealed)`, `fx::take_action_card_staged`, `Spawn::DrawCards` count 0 for the hand limit) | `faction_dual::mageon_implants`, `factions::yssaril::mageon_implants` | `factions::yssaril::mageon_implants` |
| Arborec agent (Letani Ospha) | `leader_windows::LetaniOspha` (`LAsk::OsphaShip/OsphaReplace`) | `leaders_core::arborec_agent`, `factions::arborec::agent_use` | `factions::arborec::agent` |
| Naaz Fabrication (relic, token) and Black Market Forgery | `relic_windows::Fabrication` (`RAsk::FragmentType/TokenPool`) | `faction_dual::naaz_fabrication`, `factions::naaz::fabrication` | `factions::naaz::fabrication` |
| Ghost Wormhole Generator | `technology_windows::WormholeGenerator` (`TAsk::WormholeGenerator`, `TOpt::Token`) | `technology_dual::wormhole_generator`, `factions::ghost::perform_component` | `factions::ghost::wormhole_generator` |
| Mahact Vaults of the Heir (Thunder's Edge breakthrough, cheap leaf) | `faction_component::vaults_of_the_heir` | `faction_dual::vaults_of_the_heir`, `factions::mahact::perform_component` | `factions::mahact::vaults_of_the_heir` |
| Sol Orbital Drop | `faction_component::OrbitalDrop` (`Ask::Promissory(OrbitalPlanet/OrbitalDeploy)`, payment as `Spawn::Bill`) | `faction_dual::orbital_drop`, `faction_abilities::orbital_drop` | `faction_abilities::orbital_drop` |

Orbital Drop and Arborec agent use the adapter's reads for what the core does not hold: the infantry form and `can_pay` (Orbital Drop), the unit
form each seat places per ship base (Arborec agent), the destinations of the Wormhole Generator (the map's placed systems, the printed homes,
other seats' ships). Shared type changes: `choice::{promissory, relic, technology, leaders, cards}` (new variants), `render/{promissory, relic,
technology, leaders, cards}.rs`, `query::laws::{structure_allowed, structure_limit}` (moved from `rules::production`, which re-exports),
`kernel::fx::take_action_card_staged`.

Lines in `docs/ENGINE_KNOWN_ISSUES.md`: W11-design-05, W10-design-06. `string_only_component` and `string_only_leader` lose the ported ids; both
gate tests now count every remaining id under its own name.

## Still counted gates (documented bails, each counted under its own id, tested)

Action leaders (`leaders::use_leader [gate: <id>: ...]`): arborec hero and muaat agent (the production window is a window of a use, not a
child a leader can start: no `Spawn` for it; I did not merge `core-integration-7`), nekro hero (specialty planets, the active technology deck
and `technology::gain` are string reads; staged destruction), naaz hero and winnu hero (secondary or primary of strategy cards as the
continuation of a leader), empyrean hero (loop of `explore_frontier` with rollback of state, dice and rng), cabal hero, argent hero, yin hero,
yssaril hero, mahact hero (hunt, migration, landing, reveals, a combat chosen on the map).

Faction component actions (`faction_abilities::perform_component [gate: faction|<name>: ...]`): Muaat Star Forge, The Nucleus, Ember Colossus,
The Inferno, Fires of the Gashlai (placement under fleet supply and capacity: `fleet::standing` with the fleet-limit hooks), and the Thunder's
Edge pieces Saar Deorbit Barrage and Arborec Psychospore (left undecided, as ordered).

Dropped by decision (not ported): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel.

## Checks

Run: `cargo test -p ti4-engine-core --lib`, `cargo test -p ti4-engine --lib` (4678 passed at commit 6d1006fb, 0 failed, 4 ignored; the
targeted tests after it, the final full run is in the report), `tools/engine_core/check_layers.py` (0 violations), `tools/engine_core/lint_core.sh` (0 hits). Single-file mutation
checks of every core half by Haiku runners on a committed snapshot in a scratch worktree (`/root/deploytmp/core-w-l6/mutate.py`, scripts
`mut1.sh`, `mut2.sh`, results `mut1.out`, `mut2.out`).

Batch 1 (51 mutants): 46 killed, 6 survived. Closed by new tests: Mageon self-target (the old test had no card-group table, so it passed
vacuously), the no-draw count of the hand-limit child, the card-group table that does not reach the hand, the guard against taking from oneself
(`fx::take_action_card_staged`). Equivalent mutants: the Ospha cost ceiling +5 halves (ship costs are whole numbers) and the offered-pick
guard of `LetaniOspha::answer` (the driver rejects an unoffered pick before `answer` runs).

Batch 2 (29 mutants: Wormhole Generator, Vaults, Orbital Drop, the Mageon survivors): 24 killed, 5 survived. Closed by new tests: Orbital
Drop's token-event reason, a mech offer from an empty box, the payment of the mech through the `Spawn::Bill` child, the cost of 3. Batch 3
(those four again): 4 of 4 killed. Equivalent mutant: `OrbitalDrop::answer` moving to `Paying` on a decline (the idle child ends at once).

## Not run (by the directive)

`verify_package`/gate, golden or wide dual run, census re-run (the gate tables are by reading), bench, soak, mutation ratchet, workspace
clippy, board_equivalence.

## Merge notes

Likely conflicts with lane 1: none expected from files (no production or combat file touched); with lane 5: none (on top of it). The
`ErasedChild`/`Spawn::Bill` use of Orbital Drop relies on `driver::children::register`.
