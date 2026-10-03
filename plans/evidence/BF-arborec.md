# BF-arborec: The Arborec (`crates/ti4-engine/src/factions/arborec.rs`)

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `mitosis` | done (docks barred via `cannot_produce`; status-phase placement; mech replacement) | `space_docks_cannot_produce_infantry_and_only_the_arborec_are_barred`, `a_dock_alone_is_no_spot_for_infantry_but_a_letani_warrior_is_a_producer`, `mitosis_places_an_infantry_on_the_planet_you_choose`, `mitosis_is_not_a_question_with_one_planet_and_no_mech_to_spare`, `mitosis_does_nothing_with_no_planet_to_receive_it` |
| tech | `bio` | done (moves checked against the unmoved infantry; adjacency via galaxy incl. wormholes) | `bioplasmosis_*` (4) |
| tech | `lw2` | partial: rule implemented, but fires only where `GROUND_FORCE_DESTROYED` is emitted (invasion paths) | `a_destroyed_letani_warrior_ii_*`, `units_on_the_card_*`, `each_returning_unit_*`, `units_stay_on_the_card_*`, `only_the_owners_letani_warrior_ii_rolls` |
| unit | `arborec_infantry`, `arborec_infantry2` | done: data-driven stats verified (8/7, PRODUCTION 1/2, cost 0.5, upgrade picked with lw2) | `the_unit_sheet_is_what_the_cards_print`, `owning_letani_warrior_ii_places_the_upgraded_unit` |
| unit | `arborec_mech` | done (DEPLOY inside Mitosis; stats verified) | `the_behemoth_may_replace_an_infantry_instead`, `the_mech_replacement_needs_a_mech_in_the_box_and_an_infantry_to_replace` |
| unit | `arborec_flagship` | done | `the_flagship_produces_up_to_five_*`, `*_may_be_declined_*`, `*_is_not_offered_*` |
| promissory | `stymie` | done (once per SHIP_MOVED event, see questions) | `stymie_*` (4) |
| leader | `arborecagent` | done | `the_agent_*`, `the_ships_owner_may_decline_*` |
| leader | `arboreccommander` | done (unlock + effect) | `the_commander_*` (3) |
| leader | `arborechero` | done (production runs without a timing handle: no `UNITS_PRODUCED`) | `the_hero_*` (2) |
| breakthrough | `arborecbt` | done (exhaust tracked in `faction_marks`, readied at `STATUS_PHASE_ENDED`) | `psychospore_*` (2) |
| neutrality | | done | `a_game_with_no_arborec_seat_is_offered_nothing_of_theirs` |

## Hook requests

1. `GROUND_FORCE_DESTROYED` outside invasion (action cards, agendas, space cannon outside invasion): Letani Warrior II does not roll there.
2. `leader_action`/`use_leader`/`component_actions` get no `SourceSet`, resolver or galaxy: DEFAULT is assumed, and the hero's production emits no `UNITS_PRODUCED` (resolver-less `Resolving`). A `TimingContext`-with-resolver for leader uses would fix it.
3. Breakthrough exhaustion has no shared state; Psychospore uses `faction_marks["arborec:psychospore:exhausted:<player>"]`. A shared exhausted flag + status-phase ready would replace it.

## Decision sites to register (`arborec.rs`)

| Function | Choices built/asked |
|---|---|
| `mitosis` (effect closure) | 1 |
| `bioplasmosis` (effect closure) | 1 |
| `warrior_returns` (effect closure) | 1 |
| `stymie` (effect closure) | 1 |
| `agent_use` | 2 (ship, replacement) |
| `perform_component` | 2 (system, planet) |
Production questions flow through `production::produce_by_ability` (already registered).

## Rules questions

- Stymie: SHIP_MOVED fires per ship, so a holder who declines is asked again for each further ship moved in the same movement.
- Agent: "up to 2 more" implemented as cost <= replaced + 2 (cheaper allowed, same base type excluded); replacement ignores capacity/fleet pool (enforced at end of turn, 16.3/37.3).
- Bioplasmosis: "same or adjacent" measured from the system the infantry is removed from; one infantry per question.
- Psychospore: new infantry goes on a planet the Arborec controls in that system.
- Mitosis mech: replaces an infantry on any planet (not in space).
- Mech data has `productionValue` 2 in `units.json`; not verified against the printed card.
- Hero declined everywhere returns "not performed" (not purged).

## Commands and results

- `cargo test -p ti4-engine --lib -- factions::arborec`: 38 passed, 0 failed.
- `cargo test -p ti4-engine --lib -- factions::`: 235 passed, 1 failed (`saar::the_commander_unlocks_at_three_space_docks`, not mine), 1 ignored.
- `cargo clippy -p ti4-engine --all-targets`: no warnings in arborec.rs.
- `rustfmt --edition 2024 arborec.rs`: applied.
- `cargo test -p ti4-engine -q --no-fail-fast`: lib 1744 passed, 1 failed (saar, not mine); `decision_delivery_inventory` 1 failed (my new decision sites, unregistered, listed above); other binaries ok.

## Ledger

`arborec   12/12 implemented`

## Review fixes

- B1: `lw2` and `arborec_infantry2` unclaimed (partial): the destruction roll fires only where `GROUND_FORCE_DESTROYED` is emitted. Plague (`destroy_on_planet`) and the Saar hero kill Letani Warriors with no roll until the coordinator package emits the event from shared destroy paths. `arborec_infantry` stays claimed (stats verified).
- S3: hero use returns `Some(true)` once anything was produced (never `None` after partial production). `arborechero` unclaimed (partial): its production announces no `UNITS_PRODUCED` (no resolver in `use_leader`).
- N1: Mitosis mech replacement is declinable when no infantry placement exists; infantry in a space area can now be replaced (planet `None`, id `mech|<system>|space`). Test renamed `with_no_planet_to_receive_it_the_replacement_is_a_declinable_may`.
- N2 rules question: Ospha excludes replacing a ship with the same base type (and allows cheaper units); the card does not say.
- S5 hook request: offers (`leader_action`, `component_actions`) use DEFAULT sources; performance uses `context.sources`. Needs sources passed to those hooks.
- Decision sites: no new or renamed ones (Mitosis question unchanged in count; it now also appears for a lone replacement).
- Results: `factions::arborec` 38 passed; clippy clean in arborec.rs; rustfmt applied.
- Ledger: `arborec    9/12 implemented` (gaps: `lw2`, `arborec_infantry2`, `arborechero`).

## Route update (F1/F3)

- `lw2` and `arborec_infantry2` claimed: staged `GROUND_FORCE_DESTROYED` (Plague, Unstable Planet, agenda clears) reaches the roll when flushed (`a_staged_destruction_from_a_card_or_agenda_reaches_letani_warrior_ii`). Remaining path with no roll: Letani Warriors aboard a ship destroyed in space (they die with the ship; no ground-force event), and the synchronous test-only invasion entry points.
- `arborechero` claimed: ability production without a timing handle stages `UNITS_PRODUCED` (`the_heros_production_is_announced_as_units_produced`).
- New decision sites: none.
- Results: `factions::arborec` 40 passed; clippy clean in arborec.rs; rustfmt applied. Ledger: `arborec   12/12 implemented`.
