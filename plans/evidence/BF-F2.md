# BF-F2 combat and fleet routes

Writable files used: `combat.rs`, `fleet.rs`, `factions/hooks_combat.rs`, this file.

## Items

| # | Item | Status | Changes | Tests |
|---|---|---|---|---|
| 1 | Dual-form flip at space combat start/end (Package I, space half) | done (space half) | `fleet::flip_to_ship_forms` / `flip_to_ground_forms` (generic over `other_form`; only Naaz has a twin). `CombatWindow::open_round`, round 1 and not resumed: flips both sides' ground-form dual units in the active system's space area before `SPACE_COMBAT_STARTED`, `produced_hits` CombatStart and Assault Cannon; flipped forms are added to `fleet_at_start`. `announce_end`: flips ship forms back after the `destroyed` list is computed and before `SPACE_COMBAT_ENDED` is emitted. | `combat::space_routes_tests::a_ground_form_mech_in_space_takes_its_ship_form_for_the_combat_and_flips_back`, `::combats_without_a_dual_form_unit_flip_nothing`, `fleet::bf_f2_tests::space_combat_forms_flip_only_dual_form_units` |
| 2 | `CombatHooks::space_cannon_barred(state, content, sources, shooter, target_owner, system) -> bool` | done, hook empty | Dispatcher `hooks_combat::space_cannon_barred` (any true bars). `combat::space_cannon_offense` `may_fire` now also checks `cannon_barred` (active player's guns target the opponent, every other gun targets the active player); `reaching_guns_by` uses the same closure, so adjacent guns are covered. | `combat::tests::a_module_bar_silences_space_cannon_against_the_named_owner_only`, `::a_module_bar_also_stops_adjacent_reaching_guns` |
| 3 | Naalu `hcf2` half-ship charge | done, hook empty | `CombatHooks::fighter_fleet_weight_halves(state, content, player, unit_type) -> bool` (any true). `fleet::standing_with` gained a `half_weight` argument; `fleet_charged = present + ceil(fighters_charged / 2)` when an upgraded fighter type in play reports half. Capacity-only callers pass a never-half closure (they do not read the charge). | `fleet::bf_f2_tests::half_weight_fighters_charge_the_pool_rounding_an_odd_half_up`, `::half_weight_fighters_fit_twice_as_many_in_the_pool_and_the_fifth_is_over`, `::the_hook_does_not_touch_capacity_or_base_fighters` |
| 4 | Flush API | already public, no change | `combat::announce_staged_destructions(state: &mut GameState, ctx: &mut Resolving<'_>)` (combat.rs, drains `state.pending_destructions`, announcing `SHIP_DESTROYED` until empty). The coordinator already wraps it as `Game::announce_staged_destructions`. | existing `combat` tests of staged destruction |

## Rules recorded

- Item 3 rounding: the pool is compared in whole ships, `present + k/2 > limit` is equivalent to `present + ceil(k/2) > limit` for integer limit, so an odd half rounds up and the comparison is exact (k = 3 half-fighters is 1.5 ships, charged 2). `fighters_charged` stays the whole fighter count (`fighters_over_capacity` unchanged), only `fleet_charged` is halved. `enforce` removes only non-fighter ships, as before.
- Item 1: only units that have a twin whose form differs flip; this is per unit, both sides of the combat. No combat means no flip (a ground-form mech alone in space is not a ship, so no combat starts around it). A space-form mech that retreated out of the system is not flipped (the card names the active system). Flipping back happens only at the end of a battle, not when the form was made elsewhere without a combat.
- Item 1 landing half (space form committed to a planet) is not done: invasion.rs is outside this package.

## Requests for the coordinator / faction owners

1. `argent.rs`: set `hooks.combat.space_cannon_barred` (shooter != target_owner, target_owner has `argent_flagship` in `system`), claim `argent_flagship`.
2. `naalu.rs`: set `hooks.combat.fighter_fleet_weight_halves` (player is Naalu and `unit_type == "naalu_fighter2"`), claim `hcf2` and `naalu_fighter2`.
3. `naaz.rs`: claim `naaz_mech`/`naaz_mech_space` for the space half once reviewed; the landing flip and `sc` ground round start remain.

## New decision sites

None (no `Choice` built or asked in the files touched).

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- combat:: fleet:: factions::` | 386 passed, 0 failed, 1 ignored |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1845 passed, 0 failed, 1 ignored; `decision_delivery_inventory` 2 passed, 2 failed (`every_indirect_producer_reaches_its_classified_delivery_api`, `every_producer_and_delivery_site_matches_the_reviewed_registry`: unregistered sites from other concurrent packages, none from these files); other binaries ok. An earlier run in this session also showed two lib failures in `leaders.rs` and `production.rs::bf_f3_tests` (other agents, since green) |
| `cargo clippy -p ti4-engine --all-targets` | no new warning categories in my files; `space_cannon_offense` too-many-lines (112/100) and the `&may_fire` / redundant-closure notes inside it predate this work (the function was already near the limit; I moved the bar logic into `cannon_barred` to limit growth) |
| `rustfmt --edition 2024` on combat.rs, fleet.rs, hooks_combat.rs | applied |
