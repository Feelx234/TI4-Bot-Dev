# BF-BORROWED-COMMANDERS-A: acquired commanders (Obsidian, Nekro, Deepwrought, Bastion)

Status: 2026-10-05. Nothing compiled or run (cargo forbidden for this job); code and tests are
unverified until the coordinator builds. Gate for all: `crate::promissory::has_commander_ability`.

| Commander | Status | Notes |
|---|---|---|
| `obsidiancommander` | implemented, needs 1-line dispatch | `borrowed_commanders::unit_roll_modifier` |
| `nekrocommander` | blocked: no event/seam | spec below |
| `deepwroughtcommander` | blocked: seam is table-driven, in another file | spec below |
| `bastioncommander` | blocked: Sabotage gate is in reactions.rs | spec below; Nekro half vacuous |

## 1. Obsidian

Text: "At any time: Apply +1 to the result of each of your units' combat rolls in The Fracture."

Route: per-unit roll shift, same shape as `winnu.rs::unit_roll_modifier`, which `combat.rs:221`
(space) and `invasion.rs:231` (ground) read through `factions::unit_roll_modifier`. The function
returns 1 when `state.fracture_in_play`, `fracture::is_fracture_system(content, sources, system)`
(same test as `choice.rs:1051`) and `has_commander_ability(state, unit.player, "obsidiancommander")`.
Both callers pass `Some(system)`.

**Needed change (factions/mod.rs, `unit_roll_modifier`, ~line 586):** that function only sums
`MODULES` hooks, and `borrowed_commanders` is not a module. Replace the body with:

```rust
    hooks()
        .filter_map(|h| h.unit_roll_modifier)
        .map(|f| f(state, content, sources, unit))
        .sum::<i64>()
        + borrowed_commanders::unit_roll_modifier(state, content, sources, unit)
```

Then delete the `#[allow(dead_code)]` on the function in `borrowed_commanders.rs`.

Pool note: `yin.rs::yinbt_canonical_commanders` skips `faction == "obsidian"` (Obsidian is folded into
the `firmament` family, whose commander id is `firmamentcommander`), so Yin's random pool never
grants `obsidiancommander`; the handler is implemented per the operator decision but only reachable
by a direct grant. `firmamentcommander` (scoring planets in systems with your ships) is NOT in this
package.

Tests (module `tests::obsidian`):
- `obsidian_grant_adds_one_to_own_rolls_in_the_fracture_only` (space + ground, outside Fracture 0,
  other player 0)
- `obsidian_is_inert_without_the_ability_or_before_the_fracture_is_in_play`

These call the function directly; once the dispatch above exists, an end-to-end assertion through
`factions::unit_roll_modifier` should be added.

## 2. Nekro (blocked)

Text: "After you gain a technology: You may draw 1 action card."

No typed event exists for gaining a technology. `technology::grant` (technology.rs:1083) is
state-only and is called from `complete_research` (research; no timing context), `action_cards.rs`
3592 (a card grant), `exploration.rs:458` (relic/exploration grant), `factions/muaat.rs:551`
(war sun), and the Research Agreement branch of `complete_research` (holder gain). The ENTROPIC_SCAR
log string at game.rs:3998 is a label, not a window. Researching and gaining are distinct (see the
doc on `grant`); "gain" covers both, so the event must fire for both.

**Needed seam:** a `TECHNOLOGY_GAINED` typed event with payload `{player, technology, how}`, emitted
after each of the sites above through a context that carries a resolver (`Resolving::emit` with a
timing handle, or `Game::emit_typed`). The state-only `technology::research`/`complete_research`
callers (`strategy_cards.rs` 403/471/576, `action_cards.rs` 3302/3737/5917, `sardakk.rs` 1147) hold
`state`, `table` but no resolver; the simplest route is to make each caller emit after a successful
research/grant, or have `complete_research` return the list of gained aliases (including the
Research Agreement holder's) so callers emit. Add the id to `reactions::EMITTED_EVENTS` if that
registry is the arming list.

**Then in `borrowed_commanders.rs`:** an `Ability::stateful(format!("leader:{owner_name}:nekrocommander:TECHNOLOGY_GAINED:after"), seat, "TECHNOLOGY_GAINED", Relation::After, ...)`, `.with_optional(true)`, condition `event.text("player") == Some(seat) && has_commander_ability(.., "nekrocommander")`, effect: the draw routine that applies draw hooks (as in the titans handler: one atomic effect, no mutation before the ask).
Not implemented: an ability on an event that is never emitted would be an inert claim.

## 3. Deepwrought (blocked)

Text: "When another player spends resources to research a technology: That player may reduce the cost
by 1; if they do, gain 1 commodity or convert 1 of your commodities to a trade good."

Research payment path: `strategy_cards.rs::paid_research` (line 900) is the only place resources
are spent to research (Technology secondary and the primary's paid second research). It asks
`doctor_sucaban` (623) first, then `payment::plans`/`payment::apply` (966-976). It runs with
`state, content, sources, galaxy, table`, with no resolver, so no timing window opens. Doctor
Sucaban is the existing in-file template for "when a player spends resources to research".

**Needed change (strategy_cards.rs):** a sibling `deepwrought_commander(state, content, sources, galaxy, table, player, cost) -> Result<Adjustment, IllegalChoice>` called right after `doctor_sucaban` (before the affordability gate; same rollback discipline using `adjustment.rollback`-style checkpoint):
1. holder = the seat for which `has_commander_ability(state, holder, "deepwroughtcommander")` holds and `holder != player` (iterate `state.players` in `initiative_order`; one offer per holder);
2. if `cost >= 1`, ask the researcher (`Choice` to `player`, options `yes` / `decline`, source `DecisionSource::Content("deepwroughtcommander")`) whether to reduce the cost by 1;
3. on yes: `cost -= 1`; then the cardholder (`you`) gains 1 commodity (only if below capacity) or converts 1 of their own commodities to a trade good (only if they hold one), asked as a choice with a legal-options-only list; if neither is legal the reduction is still offered (the printed gain is then simply unavailable). Rules question for the operator: the printed text says "gain 1 commodity or convert 1 of your commodities", read here as the cardholder gaining; the alternative reading (the researcher gains) is not adopted.
4. The reduction and gain must roll back with the research if it does not land (like `adjustment.rollback`).

Tests to add with that change: holder with the grant, researcher reduces (cost reduced by exactly 1 and commodity/trade good change), researcher declines (no change), holder without ability (no offer), researcher is the holder (no offer).

## 4. Bastion (blocked; Nekro half vacuous)

Text: "At any time: Your action cards cannot be canceled by 'Sabotage' action cards. The Nekro Virus
cannot place assimilator tokens on your components."

Sabotage cancel route: `reactions.rs` `window_for` table row "When another player plays an action
card other than 'Sabotage'" (line ~295) uses guard `another_players_card_is_not_sabotage`
(line 160), consumed by `playable_now` (578). The Sabotage holder is offered the card only through
`playable_now`; the cancel itself is `event.cancel()` at reactions.rs ~868-873.

**Needed change (reactions.rs, `another_players_card_is_not_sabotage`, line 160):** the guard has
`state`, `event.text("player")` is the card's player. Add:

```rust
    actor_is_not(event, player, state)
        && !is_sabotage_play(event)
        && !event.text("player").is_some_and(|who| {
            crate::promissory::has_commander_ability(state, &PlayerId::new(who), "bastioncommander")
        })
```

This removes Sabotage from `playable_now`, so it is never offered against a Bastion player (an
atomic prevention, no cancellation to undo). Check whether the guard is reused by other windows
(e.g. a Sabotage-like card) and whether Sabotage-from-hand alternatives (Gilded Scales,
Wormhole-style `sabo*` plays via `action_cards.rs` 6219, Garbozia salvage) go through
`playable_now`; if any alternative route calls the Sabotage effect directly, gate it the same way.
Do not apply the gate to the faction-technology cancel at reactions.rs ~1036 (it cancels "any card,
Sabotage included"; this is a technology, not a "Sabotage" action card, so the commander does not
stop it).

Tests there: Bastion-granted player plays a card: another player holding Sabotage is not offered it
(`playable_now` empty); no grant: offered; the Bastion player's own Sabotage window unaffected.

Nekro half: "The Nekro Virus cannot place assimilator tokens on your components." There is no
Nekro module and no assimilator token in the engine (`grep -rn assimilator` finds none), so the
clause is vacuous and needs no handler. If a Nekro module is later added, its token-placement site
must check `has_commander_ability(state, target_owner, "bastioncommander")`.

## Decision sites to register

None (the only choices in this file remain the existing Titans one; Obsidian is a pure modifier).

## Commands run

`rustfmt --edition 2024 crates/ti4-engine/src/factions/borrowed_commanders.rs`. No cargo.
