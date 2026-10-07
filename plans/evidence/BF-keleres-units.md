# BF-keleres-units: Keleres flagship, mech, heroes, Riders

Files: `factions/keleres_units.rs` (all logic and tests), plus small call-ins in `tactical.rs`
(`activatable_with`), `invasion.rs` (`commit_options`, `commit_ground_forces`, `InvasionWindow::resolve`
Committing arm) and `action_cards.rs` (`predicted_outcome` made `pub(crate)`, `rider_payoff` arm).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| unit | keleres_flagship (Artemiris) | done | artemiris_* (3) |
| unit | keleres_mech (Omniopiares) | done | omniopiares_* (4 + window) |
| leader | keleresherokuuasi (keleresa) | done | kuuasi_* (3) |
| leader | keleresheroodlynn (keleresx) | done | odlynn_* (4) |
| leader | keleresheroharka (keleresm) | done in code; routing needs the HOOKS lines below | harka_* (3) + ignored routed test |
| promissory | riderm / riderx / ridera | done | a_rider_*, a_wrong_rider_* |

Neutrality: `games_without_keleres_see_nothing_of_the_units_heroes_or_riders`.

## Hook requests (keleres.rs `HOOKS`, not mine)

Add to the shared `HOOKS`:

```rust
    leader_action: Some(super::keleres_units::leader_action),
    use_leader: Some(super::keleres_units::use_leader),
```

Both return `None` for any leader but Harka Leeds; if keleres.rs gets its own `use_leader`/`leader_action`
for the agent, chain: try its own, then `keleres_units::...`. Then remove the `#[ignore]` on
`harka_is_offered_and_resolved_and_purged_through_the_shared_leader_path` and run it.

## Decision sites to register

`keleres_units.rs`: `ask` (Choice 1, AskObserved 1): Kuuasi's composition question.
(Other questions go through existing sites: `pay_seeing`, `predicted_outcome`, `gain_tokens`.)

## Rules questions / decisions

* Omniopiares: toll is once per planet per activation (one "commit ground forces to the planet"
  step), not per unit or per landing; several mechs on the planet still cost 1.
* Artemiris: gated at option generation (`activatable_with`), paid on the `SYSTEM_ACTIVATED` window
  (activator chooses planets). Token is spent before the payment; the gate guarantees it is payable.
* Kuuasi: offered to combatants only (text does not say so, but a non-combatant's ships cannot
  fight). Flagship placed only if in reinforcements; escorts "up to" 2 cruisers/destroyers.
* Odlynn: "up to 6" is taken as exactly 6 (via `vote::add_votes`, counted only if the Keleres casts
  votes). "Each player" is read as each other player; a player barred from voting abstains. Payout
  reads `agenda_votes` in the `AGENDA_RESOLVED` window.
* Rider: the note returns to the Keleres at play time (not after resolution) so a discarded agenda
  cannot strand it; the prediction lives in `agenda_predictions` as `outcome|keleres_rider`.
  Draw goes through `action_cards::draw`.
* Harka: reveals from the top; non-component cards plus the rest of the deck are shuffled with the
  game RNG (`domain::ACTION_CARDS`). Not offered when the deck holds no component-action card.

## Commands and results

* `cargo test -p ti4-engine --lib -j1 keleres`: 24 passed, 0 failed, 1 ignored (the routed Harka test).
* `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12`: lib 2410 passed, 0 failed,
  2 ignored; content_ids_resolve 1 passed; decision_delivery_inventory 3 passed, 1 FAILED (expected
  registry: `keleres_units.rs ask`, plus the other agent's `keleres.rs` sites and
  `production.rs agency_supply_network`); remaining binary 5 passed.
* `rustfmt --edition 2024` run on `keleres_units.rs` only (`action_cards.rs` has unrelated pre-existing
  diffs; `tactical.rs`/`invasion.rs` clean).

## Ledger

keleresm/x/a: 11/13 implemented; missing only `keleresagent`, `kelerescommander` (the other agent's).
