# BF-F3: production, exploration, technology and trade-goods routes

Branch `wp/base-factions`. Scope: `production.rs`, `supply.rs`, `exploration.rs`, `technology.rs`, `strategy_cards.rs`,
`relics.rs`, `factions/hooks_economy.rs`, `factions/hooks_strategy.rs`. No faction file needed `..XHooks::NONE` edits. Hooks are
neutral when empty (tests prove it). Not committed.

## Item status

| # | Item | Status |
|---|---|---|
| 1 | extra production + cost reduction | done |
| 2 | explore extra draw + `explored` + `PLANET_EXPLORED` | done |
| 3 | trade-goods routes | done in my files; remaining sites listed below |
| 4 | per-unit max cost on ability production; stage `UNITS_PRODUCED` | done |
| 5 | Yin research hooks | done |
| 6 | Mentak unit-form override | done in production.rs and technology.rs; other sites listed |
| 7 | `STRATEGY_TOKEN_SPENT` | helpers done; no spend site in my files; sites listed |

## Signatures

`EconomyHooks` (new fields, at the end):
- `extra_production: Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId, &SystemId) -> i64>`: system-level PRODUCTION. Added in `production::capacity`
  (via `production::module_production`). A positive value makes the space area a placement spot (ships already always place in space; ground forces and
  structures too, as for any producer with no planet, 68.4).
- `extra_production_planet: Option<fn(.., &SystemId, &PlanetId) -> i64>`: per planet that holds any of the player's units in `system` (keys of
  `planet_units`). Added to `capacity`; a planet with a positive value is a spot in `placements` and passes the same planet gates (Holy Planet, DMZ,
  space stations, structure limit). For Argent Hololattice return > 0 only for planets with the owner's structures.
- `production_cost_reduction: Option<fn(&GameState, &PlayerId, &SystemId) -> i64>`: summed, clamped at 0, seeded into `ProductionWindow::discount_remaining`
  in `open` and re-added in `refresh`. It is part of the combined-bill discount pool (like Sarween Tools), so it spends once per use and never goes below zero.
- `explore_extra_draw: Option<fn(&GameState, &ContentStore, SourceSet, &PlayerId, &PlanetId) -> bool>` (any module's true).
- `explored: Option<fn(&mut GameState, &ContentStore, SourceSet, &PlayerId, &PlanetId)>`.

`StrategyHooks` (new fields, at the end): `extra_prerequisite_colours(&GameState,&ContentStore,&PlayerId) -> Vec<(String, usize)>` (track name or
colour; green=BIOTIC, yellow=CYBERNETIC, blue=PROPULSION, red=WARFARE; unknown ignored), `research_waiver_offer(.., &TechnologyId) -> Option<ResearchWaiver>`,
`research_waiver_paid(&mut GameState, &ContentStore, &PlayerId, &TechnologyId)`, `unit_form_override(&GameState,&ContentStore,SourceSet,&PlayerId, base_type: &str, chosen_id: &str) -> Option<UnitTypeId>`
(first `Some` wins). New public type `hooks_strategy::ResearchWaiver { id, label }`.

Production API: `ProductionWindow::with_max_unit_cost(Option<i64>)` (builder, applied in `build_options` against the unit's printed `cost()`),
`production::produce_by_ability_capped(.., limit, max_unit_cost)`; `produce_by_ability` and `for_ability` keep their signatures (cap = None).

## Call sites

- `production::capacity`, `placements`, `ProductionWindow::open`/`refresh`/`build_options`, `buildable_for` (unit_form_override).
- `exploration::explore_drawn` (extra draw: draws a second card from the same deck, asks the explorer through the existing `ask` helper, drops the unchosen
  card; the engine keeps no exploration discard pile, so "discard" removes it from the deck; a discarded fragment is not gained) and `explore_with`
  (end: `hooks_economy::explored`, then the typed `PLANET_EXPLORED` event: `player`, `planet`, `deck`, `card`; only for a planet explore, not frontier).
- `technology::prerequisites_met` (extra colours), `can_research` (also true when a waiver is offered), `research` (a module waiver is used when prerequisites
  are unmet and Inheritance Systems is not ready; this path has no table, so the first offer is taken and `research_waiver_paid` charges it; Inheritance wins
  when both are possible), `apply_unit_upgrades` (override), public `technology::research_waiver_offer` for callers that can ask.

## Staging and the flush function

`supply::stage_event` keeps a typed event in `GameState::faction_marks["staged:event:NNNNNN"]`. It stages only when a seated player's faction has a
registered module (`supply::staging_enabled`), so a game with no module seat records nothing. Staged: `UNITS_PRODUCED` (production with
`Resolving::timing == None`), `TRADE_GOODS_GAINED` (`gain_trade_goods_staged`), `STRATEGY_TOKEN_SPENT` (`spend_strategy_token_staged`).

**Flush function the coordinator must call:** `supply::flush_staged_events(state, ctx) -> usize` with a `Resolving` that has a timing handle, after
leader actions, component actions, strategy-card primaries, relic uses and tech start-of-turn windows. With no timing handle it does nothing. Order is staging order.
Helpers: `supply::staged_events`, `supply::staged_event_types`. Until it is wired, staged marks sit in `faction_marks` for module-seat games.

Trade-goods sites converted: exploration (`ent`/`minent`/`majent` gains, `cm` Core Mine, Abandoned Warehouses and Merchant Station conversions) announced
through `gain_trade_goods_announced` (they hold a `Resolving`); Trade primary and the Dynamis Core / Stellar-Converter-refund relic gains staged
(`gain_trade_goods_staged`); Psychoarchaeology (`technology.rs` start_turn) uses the named `gain_trade_goods` with no event (no timing handle; the
coordinator can stage it by changing it to `gain_trade_goods_staged`). There are no production refunds in the engine. Commodity conversion by explore cards is announced as a gain.

## Remaining `trade_goods +=` sites (other files, for the coordinator)

action_cards.rs:2081, 2280, 2304, 3330, 3976, 5270, 5587; agenda_effects.rs:817, 889, 965, 1180; combat.rs:1830; diplomacy/transfers.rs:75; draft.rs:95;
factions/arborec.rs:1673; faction_abilities.rs:406, 409; faction_techs.rs:59, 329; game.rs:1933, 4549, 4576; laws.rs:249; legendary.rs:885, 888;
promissory.rs:511; transactions.rs:476.

## Other sites for item 6 (unit-form override)

`action_cards.rs:4684` (`placed_unit_id`), `factions/arborec.rs:136`, `factions/yin.rs:92` (each calls `ti4_content::units::unlocked_upgrade` directly);
also any `fleet`/supply reader that maps base type to the player's unit id. `mentak_cruiser3` needs no `requiredTechnology` change: the hook returns it.

## Item 7: strategy-token spend sites (not in my files)

`strategy.rs:382` (secondary follow; the card-secondary spend), `agenda_effects.rs:1038` (pool chosen), `entropic_scars.rs:174`, `faction_techs.rs:114, 325`,
`factions/muaat.rs:368, 461`, `factions/naalu.rs:460`. Replace each `spend_token(TokenPool::Strategic)` with `supply::spend_strategy_token_announced`
(has a `Resolving`) or `_staged`. `strategy_cards.rs` secondaries spend nothing themselves (the follower window in `strategy.rs` charges the token).
Reasons used by convention: short labels, e.g. "secondary", "orbital_drop".

## New decision sites

None. The Distant Suns choice goes through the existing `exploration::ask` helper; no new `Choice` builders were added. `tests/decision_delivery_inventory.rs`
is still red for other agents' unregistered sites (`leaders.rs::use_leader` count 7 and the faction `ask` helpers).

## Rules questions

- Distant Suns: the second card is drawn before the explorer chooses; if the deck holds one card only it resolves.
- Unit-form override in `apply_unit_upgrades` applies to every board unit of that base type regardless of which card the unit was (as with upgrades).
- Cost reduction applies to ability production too (`open` seeds it); if Particle Synthesis should not, gate it in the hook.

## Commands and exact results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- bf_f3` | `21 passed; 0 failed` |
| `cargo test -p ti4-engine --lib -- production:: supply:: exploration:: technology:: strategy_cards:: relics:: factions::` | `501 passed; 0 failed; 1 ignored` |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib `1845 passed; 0 failed; 1 ignored`; `decision_delivery_inventory`: `2 passed; 2 failed` (`every_indirect_producer_reaches_its_classified_delivery_api`: unclassified `leaders.rs::use_leader`; `every_producer_and_delivery_site_matches_the_reviewed_registry`; neither names my files); other binaries pass |
| `cargo clippy -p ti4-engine --all-targets` | no warning in my eight files (two I introduced, `build_options` length and a test type, were fixed) |
| `rustfmt --edition 2024` on each edited file | applied |
