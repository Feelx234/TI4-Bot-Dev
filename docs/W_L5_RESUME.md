# Lent bodies, lane 5 (branch `core-w-l5`, from `core-w-l3` @ b5f85d66)

Task: finish what lane 3 left in the string as counted gates: W11 faction component actions (lobby-seatable factions), the 15 action
leaders, Military Support, `check_unlocks`, the rest of W12/W13/W14. Pure refactor: RULES_VERSION, serde output and fixtures unchanged.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!`, lockstep test and core-crate tests)

| piece | core | adapter / call site | label |
|---|---|---|---|
| Military Support (W14) | `rules::promissory_windows::PlaceUnits` + `Then::MilitarySupport` | `promissory_dual`, `render::promissory`, `promissory::turn_started` | `promissory::military_support` |
| Gurno Aggero, Saar hero (W10) | `leader_windows::ArmageddonRelay` | `leaders_core::saar_hero`, `factions::saar::hero` | `factions::saar::hero` |
| Riftwalker Meian, Ghost hero (W10) | `leader_windows::RiftwalkerMeian` (+ `SwapFacts`, writes the `MapEdit` mark) | `leaders_core::ghost_hero`, `factions::ghost::hero_swap` | `factions::ghost::hero_swap` |
| Lazax Gate Folding, Winnu (W11) | `faction_component::lazax_gate_folding` | `faction_dual`, `factions::winnu::perform_component` | `factions::winnu::lazax_gate_folding` |
| Nekro `???_ERROR_ERROR_???` repair (W11) | `faction_component::c4r_repair` | `faction_dual`, `factions::nekro_units` | `factions::nekro::c4r_repair` |
| Terraform, Titans note (W11) | `faction_component::terraform` | `faction_dual`, `factions::titans_leaders` | `factions::titans::terraform` |

| Nekro Malleon, Nekro agent (W10) | `leader_windows::NekroMalleon`, `kernel::fx::discard_action_card_staged` | `leaders_core::nekro_agent`, `factions::nekro_units::malleon_dual` | `factions::nekro::malleon` |
| Stall Tactics, Yssaril (W11) | `promissory_windows::StallTactics` (+ `PlaceUnits`) | `faction_dual::stall_tactics`, `factions::yssaril::stall_tactics` | `factions::yssaril::stall_tactics` |
| ???_ERROR_ERROR_??? draw, Nekro (W11) | `promissory_windows::C4rDraw` (+ child `DrawCards`) | `faction_dual::c4r_draw`, `factions::nekro_units::discard_and_draw` | `factions::nekro::c4r_draw` |
| Vortex, Cabal (W11) | `technology_windows::Vortex` | `technology_dual::vortex`, `factions::cabal::vortex_perform` | `factions::cabal::vortex` |

Lines in `docs/ENGINE_KNOWN_ISSUES.md`: W14-design-03, W11-design-02, W11-design-03, W11-design-04, W10-design-04, W10-design-05. The three gate tables lose these ids
(`leaders::string_only_leader`, `faction_abilities::string_only_component`); their gate tests are updated.

New counted bails (each with a test): a Saar seat whose adjacency a faction rule changes (`the_hero_stays_in_the_string_where_a_faction_rule_changes_the_adjacency`),
the Ghost hero while recorded map edits are not replayed on the map, a Nekro seat carrying C4R on a Valefar Assimilator
(`nekro_error_error_repair_runs_in_lockstep_with_the_core`).

## Already done by lane 3, found so (no work left)

`check_unlocks`, `unlockable`, `commander_unlocked` are typed reads and the `unlock` write; `award_tail` is those plus the staged Yin
breakthrough event (Thunder's Edge). Nothing is lent there.

## Still counted gates (string body, named in the census), with the reason

Action leaders (`leaders::use_leader [gate: <id>: ...]`): arborec agent (a replacement of a ship, cost ceilings and the box, another seat answers),
arborec hero and muaat agent (production windows of lane 1, W5), nekro hero, naaz hero and winnu hero (a strategy card as the child of a leader window), empyrean hero (exploration composition),
cabal hero (hunt over rifts and ships), argent hero (fleet migration over the movement windows), yin hero (invasion composition), yssaril hero
(reveal rules and the game's random stream), mahact hero (tactical composition).

Faction component actions (`faction_abilities::perform_component [gate: ...]`): orbital drop (Sol; payment child and a placement of a mech), muaat
(Star Forge: placement and production of lane 1), ghost Wormhole Generator (a Creuss token is a state of the string map: the core has no
window that writes it), saar Deorbit Barrage and arborec Psychospore (Thunder's Edge breakthroughs: undecided, nothing ported), titans
(none left), naaz (fabrication/BMF: relic gain and fragments), yssaril (Mageon Implants: the look-and-take of another seat's hand), arborec Psychospore (TE), nekro C4R place PDS (structure cap), mahact (Imperia).

Next in line, cheapest first: C4R place PDS (structure cap and placement under the box), Mageon Implants, the Arborec agent, Naaz fabrication.

Not mine: W12 Chaos Mapping / Minister of Industry / Freelancers and W13 `frln*` (lane 1's production window), the exploration reward
composition (D1), `control_gained` (Keleres: dropped), `technology::component_actions`/`perform_component` (D1b dispatch).

## Checks

Run: `cargo test -p ti4-engine-core --lib` (343+), `cargo test -p ti4-engine --lib` (4671 passed, 0 failed, 4 ignored), the targeted
lockstep tests named above, single-file mutation checks by Haiku runners on a committed snapshot in a scratch worktree
(`/root/deploytmp/core-w-l5/mutate.py`, specs `mut1.json`, `mut1b.json`, `mut2.json`, `mut3.json`; results `mut*.out`).

Equivalent mutants (the driver rejects a pick that is not among the offered options before `answer` runs): the optional check and the
spot check of `PlaceUnits::answer`, the first-system check and the second-differs check of `RiftwalkerMeian::answer`; the box check of Lazax
Gate Folding for the infantry (uncapped: no plastic); the Mecatol system fact of Lazax Gate Folding (the fixtures only have the base Mecatol tile).

Not run (by the directive: no heavy work before the end of D1): `verify_package`/gate, golden or wide dual run, census re-run (the counted
gates above and the census tables of `LENT_CENSUS.md` are by reading, not measured), bench, soak, mutation ratchet, workspace clippy,
board_equivalence.

## Merge notes

Shared types touched: `choice::promissory` (was empty; area now ported), `choice::leaders` (+ `SaarHero`, `GhostSwapFirst`, `GhostSwapSecond`),
`render/mod.rs` (Promissory registered), `rules/mod.rs` (+ `promissory_windows`, `faction_component`), `ti4-engine/src/lib.rs` (+ `promissory_dual`,
`faction_dual`). Likely conflicts with lane 1: none expected (no production, combat or Letnev/Thundarian files touched); with lane 3: none (lane 5 is on top of it).
