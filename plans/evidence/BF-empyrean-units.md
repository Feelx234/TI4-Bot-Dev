# BF-empyrean-units: Empyrean flagship, mech, agent and the two pacts

Branch `wp/base-factions` (shared checkout, uncommitted). Card text from
`crates/ti4-content/content/{units,leaders,promissory_notes}.json`.

## Items

| Kind | Id | Status | Route | Tests |
|---|---|---|---|---|
| unit | `empyrean_flagship` (Dynamo) | done | `SUSTAIN_DAMAGE_USED` (ships) and `GROUND_FORCE_SUSTAINED` (ground forces) after-windows, `empyrean_units::dynamo`; paid by `production::pay_seeing` | `dynamo_*` |
| unit | `empyrean_mech` (Watcher) | done | `ACTION_CARD_PLAYED` when-window, cancels the event (as Instinct Training), `empyrean_units::watcher` | `the_watcher_*` |
| leader | `empyreanagent` (Acamar) | done | `MOVEMENT_FINISHED` after-window, `strategy_cards::gain_tokens` (the mover picks the pool) | `acamar_*` |
| promissory | `blood_pact` | done | ACTION placement + return in `promissory.rs`; votes in `vote::VoteWindow::record` via `empyrean_units::blood_pact_votes` | `a_pact_*`, `blood_pact_*` |
| promissory | `dark_pact` | done | ACTION placement + return in `promissory.rs`; gain in `transactions::resolve` via `empyrean_units::dark_pact_gains` | `a_pact_*`, `dark_pact_*` |

Neutrality: `games_without_the_empyrean_see_nothing_of_the_units_agent_or_pacts`.

## Decisions

* Dynamo repairs on both sustain events. Space sustains emit `SUSTAIN_DAMAGE_USED`; ground forces
  (mechs) emit `GROUND_FORCE_SUSTAINED` (no `SUSTAIN_DAMAGE_USED` exists for them). "Adjacent" is the
  galaxy's `adjacent` (wormholes included); with no map only the flagship's own system counts.
  Choosing the ability is the consent; payment asks which planets to exhaust, and nothing changes if
  the repair cannot be paid (state restored).
* Watcher with several mechs in reach asks which to remove; one mech is not a decision (choosing the
  ability is the consent). The card is still spent: `reactions::announce` discards a cancelled card.
* Acamar is hung on `MOVEMENT_FINISHED` (one window per movement step, `ships_moved > 0`), not on the
  per-ship `SHIP_MOVED`, so it is offered once per tactical action. The mover (not the Empyrean)
  chooses the pool, like any command-token gain (`strategy_cards::gain_tokens`).
* Pacts follow their ACTION text like Trade Convoys: `promissory::take` no longer places them
  faceup; `promissory::action_notes_in_hand` / `play_action_note` place them. Return on activating a
  system holding the Empyrean's units is in `spend_support_on_activation` (the shared activation
  path), for any system containing a unit of the owner, in space or on a planet.
* Blood Pact: the pair completes with whoever votes second, so the 4 votes are banked once, with that
  vote's tally for the shared outcome. Both must have cast votes (> 0).
* Dark Pact: "maximum commodity value" is the giver's printed faction value (21.1); the gift must
  equal it exactly. Both gain via `supply::gain_trade_goods_staged(.., "dark_pact")`.

## Changes outside the assigned files (test-only, forced by the placement change)

`promissory::take` stopped placing pacts faceup, so three existing tests that relied on receipt now
place the card with its ACTION first: `combat.rs` (`note_holdings` test) and `secrets.rs`
(`rival_note_progress_deduplicates_note_kinds_from_one_issuer`, `strengthening_bonds_*`).

## Hook requests (files not mine)

1. `factions/empyrean.rs` MODULE `hooks`: add
   `component_actions: Some(super::empyrean_units::component_actions)` and
   `perform_component: Some(super::empyrean_units::perform_component)` (chain them with any of its own).
   Without them the pacts' ACTION is never offered in a game; the functions are tested directly.
2. `factions/empyrean.rs` leader claims: `LEADERS` here claims only `empyreanagent`.

## Decision sites to register

* `factions/empyrean_units.rs::ask` (1 `Choice::new`; asked by `watcher`, subtype `watcher_remove`).
  Dynamo's payment and Acamar's pool choice go through existing sites (`production::pay_seeing`,
  `strategy_cards::gain_tokens`).

## Rules questions

* Acamar's "that player gains 1 command token" names no pool; the player picks (52.4 flow).
* Dynamo's "unit ... uses SUSTAIN DAMAGE" is read to include ground forces; Nomad's
  Quantum-Manipulator-style paths that cancel a hit without the damage event are not covered.

## Commands and results

Env: `CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false CARGO_PROFILE_TEST_CODEGEN_UNITS=16`, `-j1`.

* `cargo test -p ti4-engine --lib -j1 empyrean_units`: 20 passed in my module (all `empyrean_units`
  tests); the one failure in that run was `empyrean::tests::voidwatch_*` (the other agent's file).
* `cargo test -p ti4-engine -j1 -- --test-threads=12` (lib): 2383 passed, 1 failed, 1 ignored. The
  failure is `factions::empyrean::tests::aetherstream_also_serves_...` (other agent's file, in
  flux). Cargo stopped there, so integration tests did not run in that invocation.
* `cargo test -p ti4-engine -j1 --test decision_delivery_inventory`: 3 passed, 1 failed, as expected:
  unreviewed sites `empyrean_units.rs::ask` (Choice 1, AskObserved 1), plus the other agent's
  `empyrean.rs::tether_place` and `voidwatch`.
* Ledger (`print_faction_ledger`): not completed (cargo lock contention); the claims are
  2 units, 2 promissory notes, 1 leader (`empyreanagent`).
* `rustfmt --edition 2024` run on `promissory.rs` and `empyrean_units.rs` (clean at base / mine);
  `vote.rs`, `transactions.rs` pass `--check`; `combat.rs`, `secrets.rs` hand-edited, not formatted.
