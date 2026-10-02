# Faction agent guide (BF plan, waves C–D)

You implement **one faction** in `crates/ti4-engine/src/factions/<alias>.rs`. Read this whole file,
then `crates/ti4-engine/src/factions/mod.rs` (the contract), then your faction's checklist in
`plans/BASE_FACTIONS_PLAN_2026-10-02.md`.

## Hard rules

| Rule | Detail |
|---|---|
| Writable | `crates/ti4-engine/src/factions/<alias>.rs` and `plans/evidence/BF-<alias>.md`. Nothing else. |
| Shared file needed? | Stop that item. Record in evidence under "Hook requests": file, function, the exact hook signature you need, and why. The coordinator adds it. Carry on with other items. |
| Build must stay green | Five other agents share this checkout and `target/`. Never leave `<alias>.rs` uncompilable between tool calls; write a hook body fully before registering it in `MODULE`. |
| Tests | Inline `#[cfg(test)] mod tests` in your file. Run `cargo test -p ti4-engine --lib factions::<alias>`; **0 tests run = failure**. Also run `cargo clippy -p ti4-engine --all-targets` and fix warnings in your file only. Do not run `cargo fmt` on the workspace; `rustfmt --edition 2024 crates/ti4-engine/src/factions/<alias>.rs` only. |
| Never | commit, stage, switch branch, `git stash/reset/checkout/clean`, delete files, touch `out/`, create folders, run `ti4-sim` or training. |
| Claims | Add an id to `MODULE.abilities/technologies/units/promissory/leaders/breakthroughs` only when its printed text is implemented **and** a test proves it. `factions::tests::claims_name_real_assets_of_their_own_faction` rejects ids not on your sheet. |
| Text | Card text comes from `crates/ti4-content/content/*.json` (latest printing at `DEFAULT`). Quote it in a doc comment above the code. Official rules govern edge cases; cite the rule number. |
| Unsure about a rule | Implement the unambiguous part, record the question in evidence. Do not guess silently. |

## The contract (see `Hooks` in `factions/mod.rs`)

- Hooks are called for **every** player and **every** module. Check your own condition (does this
  player have the ability / hold the note / own the tech / have the leader unlocked) and return
  the neutral value otherwise.
- **Atomic:** returning "not performed" (`false`, `None`, `Some(false)`) means the state is
  untouched. Ask every choice and check every cost before the first mutation.
- **Deterministic:** no `HashMap` iteration, no clocks, dice only through `context.dice`/`rng`.
- **Cancels and errors:** several emit sites ignore a WHEN cancel and swallow errors
  (`let _ = ...emit(...)` in `combat.rs`, `invasion.rs`, `game.rs`). Your effect must be complete
  or not happen; never rely on cancelling those events.
- **"Once per X":** the resolver's `Frequency` bookkeeping is not saved and resets when a game is
  restored or branched. Gate "once" on state (exhausted card, a `GameState` flag).
- **`timing_abilities` is called once, at game construction**, with that state. Build the same
  abilities regardless of state; decide everything in the condition/effect.
- **Games without your faction must not change.** Timing abilities are registered for every
  seat; each condition must be false unless that seat really holds the card/ability/leader in a
  usable state. An ability that offers a choice to a seat without the card breaks dozens of
  scripted tests (`ScriptDiverged`). Ability ids use the real `owner_name` argument.
- **New choice sites:** do not edit `tests/decision_delivery_inventory.rs`. List every function in
  your file that builds a `Choice` or asks one (name, count) in evidence under "Decision sites to
  register"; the coordinator registers them.
- **`unit_dice` adjusts** the running count; do not overwrite it without saying why.
- **Already implemented in shared code — do not re-implement** (claim it only after a test shows
  it works through the shared path): `unrelenting` (Sardakk, `faction_abilities::combat_modifier`);
  `naalucommander` unlock (`leaders::commander_unlocked`). Check the ledger and grep your ids
  before starting any item.
- Legal actions are *generated*: only offer an option that can resolve now.

| Hook | Use for |
|---|---|
| `timing_abilities` | Anything "when/after X": abilities, PNs, agents/commanders with a window, techs with a window, flagship/mech triggered text, breakthroughs. Returns `crate::timing::Ability` values (see examples). |
| `component_actions` + `perform_component` | "ACTION:" text: abilities, techs, PNs, relics-like faction cards. Option ids must start `faction|<alias>|`. |
| `leader_action` + `use_leader` | Agent/hero with an ACTION window (offered as a component action). Shared code exhausts the agent / purges the hero after `Some(true)`. |
| `commander_unlocked` | Commander unlock condition. |
| `combat_modifier` | Whole-player roll shift (space/ground). |
| `unit_roll_modifier`, `unit_dice` | Per-unit combat roll / dice count (flagships, upgrades, commanders tied to a system). |
| `fleet_supply`, `status_tokens`, `waived_prerequisites`, `substitutes_primary`, `secondary_is_free`, `trades_action_cards`, `ignores_neighbours`, `vote_bonus` | As named. |
| `control_gained`, `strategy_resolved`, `space_combat_round_started`, `ground_combat_round_ended` | As named. |

### Events you can hang timing abilities on

Typed events reach the resolver only through `Game::emit_typed` (game.rs) or
`Resolving::emit` (choice.rs). **Verify** the event you need is emitted that way and read its
payload keys at the emit site before relying on it: `grep -n '"EVENT_NAME"' crates/ti4-engine/src/*.rs`.
`game.rs`'s `self.emit("...")` string labels are logs, not windows. `Resolving::emit` with no
timing handle (`timing: None`) opens **no** window — check the emitting path carries one (e.g.
Scanlink explore, Dark Energy Tap and agenda resolution currently do not).

Known typed events include: `TURN_BEGAN`, `SYSTEM_ACTIVATED`, `SHIP_MOVED`, `SPACE_CANNON_HITS`,
`ANTI_FIGHTER_BARRAGE_STARTED`, `SPACE_COMBAT_STARTED`, `COMBAT_ROUND_STARTED`, `HITS_TO_ASSIGN`,
`SUSTAIN_DAMAGE_USED`, `SHIP_DESTROYED`, `RETREAT_DECLARED`, `SPACE_COMBAT_WON`, `INVASION_BEGAN`,
`UNITS_COMMITTED`, `GROUND_ROLLS_MADE`, `UNIT_ABILITY_ROLLED`, `PLANET_CONTROL_GAINED`,
`PRODUCTION_USED`, `STRATEGY_PHASE_BEGAN`, `STRATEGY_CARD_CHOSEN`, `STRATEGIC_ACTION_BEGAN`,
`ACTION_CARD_PLAYED`, `ACTION_CARD_DISCARDED`, `TRANSACTION_OPENED`, `TRANSACTION_RESOLVED`,
`AGENDA_PHASE_BEGAN`, `AGENDA_REVEALED`, `VOTES_CAST`, `AGENDA_RESOLVED`, `PLAYER_PASSED`,
`TURN_PASSED`, `ACTION_COMPLETED`, `STRATEGY_CARDS_WOULD_RETURN`. Status-phase and
round events: verify first.

If the moment you need is not emitted (or lacks a payload key), that is a hook request.

## Copy these

| Pattern | Example |
|---|---|
| Timing ability with a state condition, optional, asks a choice | `reactions.rs` `instinct_training`, `l1z1x_agent` |
| Component action with choice + atomic resolution | `faction_abilities.rs` `component_actions` / `perform_component` (Orbital Drop, Production Biomes) |
| Leader effects | `leaders.rs` `use_leader` arms, `can_resolve_action` |
| Commander unlock | `leaders.rs` `commander_unlocked` |
| Placing units from reinforcements | `crate::action_cards::place_units` (supply-capped) |
| Promissory notes: holder/owner lookups, return | `crate::promissory::{held_foreign, holder_of, give_back, note_id, faction_name}`; Political Favor in `game.rs` `offer_political_favor` |
| Choices | `crate::choice::{Choice, ChoiceOption}`, `.contextualized(DecisionContext::new(..., DecisionSource::FactionAbility("<id>".into()), "<subtype>", phase, round))`, `context.ask_seeing(&choice)` |
| Seat factions for a test | `crate::fixtures::seated_game(&[("a", "<alias>"), ("b", "sol")], DEFAULT)` — deploys home, units, techs, leaders. Do **not** overwrite `Player::faction` on `fixtures::game`. |
| Tests with a real context | `crate::fixtures::with_context(&mut state, DEFAULT, galaxy, &mut table, |ctx| ...)`; also `fixtures::{put, put_on_planet, a_placed_planet, plain_hub}` |
| Timing ability test | `let mut resolver = crate::fixtures::armed_resolver(&state);` (proves your hook is wired through `reactions::arm`), make the event with `EventSequence::next(type, payload)`, then `resolver.emit_with_context(&mut ctx, event, |_, _| {})` inside `with_context`. Scripted answers: `crate::choice::Scripted` on the `Table`. |

## Per-item definition of done

Implemented as printed → test exercising the positive case **and** the main negative case (not
offered / no effect when the condition fails) → id claimed in `MODULE` → noted in evidence.

## Evidence file `plans/evidence/BF-<alias>.md`

Sections: Items (table: kind, id, status done/partial/blocked, test names), Hook requests,
Rules questions, Commands run with exact results, Ledger line from
`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`.

## Final report to the coordinator (≤ 300 words)

Items done / partial / blocked, hook requests, rules questions, exact test counts, clippy state.
Do not claim anything you did not run.
