# BF-mahact-units (Mahact part B)

## Items
| Kind | Id | Status | Tests (factions::mahact_units::tests) |
|---|---|---|---|
| unit | mahact_flagship | done, claimed | arvicon_rex_rolls_two_better_..., arvicon_rex_decides_a_real_combat |
| unit | mahact_infantry | done, claimed | legionnaire_gains_a_commodity_or_converts_..., legionnaire_dies_in_a_real_ground_combat_and_pays, legionnaire_is_not_triggered_... |
| unit | mahact_infantry2 | done, claimed (part A may claim `cl2`) | legionnaire_two_goes_on_the_card_and_returns_... |
| unit | mahact_mech | built and tested, NOT claimed: needs game.rs call | starlancer_* (3) |
| leader | mahacthero | built and tested, NOT claimed: needs mahact.rs hooks | the_hero_* (3), neither_side_may_retreat_... |
Neutrality: games_without_mahact_see_nothing_of_the_units_or_the_hero.

## Hook requests (exact)
1. `game.rs`, right after the `offer_nullification_field` block (same shape):
```rust
if let Some(holder) = crate::factions::mahact_units::offer_starlancer(
    &mut self.state, self.content, self.sources, &mut self.table,
    self.galaxy.as_ref(), &system, &window.player,
) {
    self.tactical = None;
    self.state.active_system = None;
    self.state.pending = None;
    self.emit(&format!("TURN_ENDED_BY_STARLANCER:{holder}"));
    self.advance_turn()?;
    return Ok(self.result(true, None));
}
```
2. `mahact.rs` MODULE hooks: `use_leader_timed: Some(super::mahact_units::use_leader_timed)` and `leader_action` (chain with part A's: `super::mahact_units::leader_action(s, c, p, l).or_else(|| <part A>)`).
3. Then add `"mahact_mech"` to `UNITS` and `"mahacthero"` to `LEADERS` in mahact_units.rs.
4. Starlancer's `take_token` edits the `mahact::fleet_mark` string directly (documented format); swap for part A's mutator if one exists. Part A's reinforcement accounting must count pool tokens, else "they gain that token" duplicates one.

## Decisions / rules questions
- Starlancer: "they gain that token" implemented literally (gain into a pool of the activator's choice via `strategy_cards::gain_tokens`), not "return to reinforcements".
- Hero: "move all units" = the owner's units in the origin's space area; origin needs an owner ship, destination exactly one other seated player's ships and no neutral (two-sided combat). `leader_action` is map-free (unlocked, both sides have ships); `use_leader_timed` refuses atomically without a legal pair. Retreat and Skilled Retreat barred via `ship_movement_barred`; other ship-moving abilities (e.g. Rescue) are not gated.
- CL infantry are uncapped in `supply::plastic`, so the card is a counter (`faction_marks mahact:cl2:<player>`).

## Decision sites to register
- mahact_units.rs `legionnaire` (Choice, 1; ctx asked via ask_seeing)
- mahact_units.rs `legionnaire_returns` (Choice, 1)
- mahact_units.rs `offer_starlancer` (Choice 1, AskObserved 1)
- mahact_units.rs `use_leader_timed` (Choice 1)

## Shared-file edits
combat.rs: `resolve_resolving` (resolve now delegates), flagship bonus in `effective_from`, bar in `CombatWindow::retreats` and `skilled_retreat_destinations`. rustfmt reformatted the whole file (not clean at base): ~300 unrelated formatting lines.

## Commands
- `cargo test -p ti4-engine --lib -j1 mahact_units`: 14 passed.
- `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2469 passed; decision_delivery_inventory fails only on unreviewed sites (mine above plus part A's `ask_among`).
