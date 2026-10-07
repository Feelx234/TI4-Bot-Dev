# BF-SSRUU-MUAAT-ACTION (implementation written; tests NOT run)

Status: source change and inline tests written in `crates/ti4-engine/src/factions/muaat.rs`.
No Cargo was run in this package (root/Codex owns builds and test execution), so **no test in this
file has been executed and nothing here is evidence that the change passes.** Root acceptance and
the independent tier C review are pending.

This file was previously a premature draft describing an `umbat_available` helper and two tests that
did not exist in the tree. It is rewritten from the actual diff. The earlier draft is not evidence
of an implementation.

## Objective

Ssruu (Yssaril `yssarilagent`, Clever Genome: "This card has the text ability of each other
player's agent, even if that agent is exhausted.") may use a seated Muaat player's Umbat agent
text, including when that source agent is exhausted. Both factions are in scope; no Crimson or
other external commander was added.

## Printed source text

`crates/ti4-content/content/leaders.json`:

- `muaatagent` (Umbat), `abilityWindow: "ACTION:"` — "Exhaust this card to choose a player: that
  player may produce up to 2 units that each have a cost of 4 or less in a system that contains one
  of their war suns or their flagship."
- `yssarilagent` (Ssruu), `abilityWindow: "At any time:"` — "This card has the text ability of each
  other player's agent, even if that agent is exhausted."

## Permission scope

- Class: P1 (source + inline tests + this evidence file only).
- Writable and written: `crates/ti4-engine/src/factions/muaat.rs`,
  `plans/evidence/BF-SSRUU-MUAAT-ACTION.md`.
- Read-only: `crates/ti4-engine/src/leaders.rs`, `crates/ti4-engine/src/factions/hooks_cards.rs`,
  `crates/ti4-engine/src/factions/mod.rs`, `crates/ti4-engine/src/production.rs`,
  `crates/ti4-engine/src/fixtures.rs`, `crates/ti4-content/content/leaders.json` and everything else.
- No Cargo, no subagents, no Git mutations, no folders, deletions, network, servers, settings or
  unrelated edits. Shared checkout `wp/base-factions` HEAD `cadbbe2d`, intentionally dirty; every
  pre-existing edit was preserved. `rustfmt --edition 2024` was run on `muaat.rs` only; it changed
  one hunk of this package's own new code and left the rest of the file byte-identical.

## What was actually wrong

`leader_action` gated on `is_muaat(state, player) && leader_status(state, player, AGENT) ==
Some(Readied)`, and `umbat` rejected `!is_muaat(context.state, player)`. A Ssruu borrower is not
Muaat and does not hold `muaatagent`, so `leaders::borrowed_component_actions` never offered the
copy (`factions::leader_action` answered `Some(false)` for it) and `leaders::use_leader_text` →
`dispatch_leader` → `factions::use_leader` → `umbat` refused it even if asked directly.

## Change (all in `crates/ti4-engine/src/factions/muaat.rs`)

1. **New private `umbat_available(state, content, player) -> bool`** (Umbat section, immediately
   before `leader_action`). One boundary for both rights:
   - native: `is_muaat(state, player)` **and** own `AGENT` status `Readied`; or
   - copied: `crate::factions::hooks_cards::borrowable_agents(state, content, player)` contains a
     `(source_seat, muaatagent)` pair for this caller. The shared contract already requires the
     caller to hold a **readied** `yssarilagent` and the copied card to belong to another seat,
     `Readied` **or** `Exhausted`; nothing here re-derives or widens it.
   The module never readies or exhausts the source agent and never changes a seat's faction.
2. **`leader_action`** (the offer gate used by `leaders::component_actions` and
   `leaders::borrowed_component_actions`) now calls `umbat_available` in place of the native-only
   condition. The existing "some player can actually produce" requirement
   (`!agent_targets(...).is_empty()`) is unchanged, so the offer and the effect use the same
   boundary.
3. **`umbat`** (the effect) now rejects `!umbat_available(context.state, content, player)` in place
   of `!is_muaat(context.state, player)`. Its body is untouched: player choice, single-system
   shortcut or system choice, then `crate::production::produce_by_ability_capped(..., Some(2),
   Some(4))`. Choice controller stays the caller (the borrower when borrowed); produced-unit owner
   and payer stay the chosen player. The two-unit cap and the printed cost limit of four are the
   same call arguments as before.

Unchanged and relied upon, not edited here: `leaders::use_leader_text` (validates the copy through
`borrowable_agents`, runs the source dispatch as the borrower, exhausts only Ssruu on success, then
runs `borrowed_agent_used`), `leaders::component_actions` / `borrowed_component_actions` (option id
`component|leader|yssarilagent|<agent>`), `leaders::can_resolve_action`,
`hooks_cards::borrowable_agents`, `agent_systems`, `agent_targets`, `ask`, `use_leader`.

## Tests added (inline `#[cfg(test)]` in `factions/muaat.rs`, new "Umbat copied through Ssruu"
section)

Fixtures are the existing ones: `fixtures::seated_game`, `fixtures::with_context`, `scripted`,
`never`, `set_status`, `count`, `home_of`, `leader_status`. New local helpers: `borrow_game`
(`a` = Muaat holding Umbat, `b` = Yssaril holding a readied Ssruu — both agents start readied via
`leaders::deploy`), `component_ids`, `borrow_umbat` (calls `leaders::use_leader_text`, not the
module hook), `standing` (units per type in a system, to measure what production actually did).

- `ssruu_is_offered_umbat_as_a_borrowed_component_action` — `leaders::component_actions` for the
  borrower contains `component|leader|yssarilagent|muaatagent`; the borrower is **not** offered
  `component|leader|muaatagent`; the Muaat seat keeps its native offer and is offered nothing
  naming `yssarilagent`; `leader_action` answers `Some(true)` for the borrower.
- `ssruu_uses_umbat_to_make_another_seat_produce_and_only_itself_exhausts` — for **both** source
  states (`Readied` and `Exhausted`): the borrower `b` chooses the *other* seat `a`; `a` gains a
  cruiser in `a`'s own home system (which holds `a`'s war sun), `b`'s home is unchanged; the bill
  is paid from `a`'s world (`exhaust|muaat` is answered and `muaat` ends in
  `state.exhausted_planets`), `a`'s and `b`'s trade goods are unchanged; the source agent is left
  exactly as it was and only `yssarilagent` is `Exhausted`.
- `the_copied_umbat_keeps_the_two_unit_cap_and_the_cost_four_limit` — source agent exhausted, the
  target given 10 trade goods so the purse is not what stops the spending; only the target choice is
  scripted and the first-option decider spends as much as it can; asserts the measured unit delta is
  `> 0`, `<= 2`, and that every newly added unit type has corpus cost `<= 4.0` (so no war sun or
  flagship), and that the borrower paid nothing.
- `a_borrow_without_ssruu_or_without_a_copyable_source_changes_nothing` — Ssruu exhausted, Ssruu
  removed, and source `muaatagent` `Locked`: each gives `leader_action == Some(false)`, no offered
  component id containing `muaatagent`, and `leaders::use_leader_text` returning `false` with the
  whole `GameState` equal to its checkpoint (`never()` table: nothing is asked). Also asserts the
  borrower is still Yssaril and never holds `muaatagent`.
- `the_native_muaat_seat_still_uses_and_exhausts_its_own_umbat` — the native path through
  `leaders::use_leader` still produces and exhausts `muaatagent`, leaves Ssruu `Readied`, and an
  exhausted native agent is **not** offered to its own seat merely because another seat holds Ssruu.

Existing native tests (`umbat_lets_the_chosen_player_produce_two_units_costing_four_or_less`,
`umbat_needs_a_ready_agent_a_player_who_can_produce_and_a_choice`) were left untouched; they are
the regression guard for the native readied/exhausted/decline/no-war-sun/foreign-leader cases.

Real fixture IDs are used, not invented ones: `build|cruiser|1` and `exhaust|muaat` (the Muaat
homeworld, resources 4, the only spendable face for that seat), player id `a`,
`component|leader|yssarilagent|muaatagent`.

## Not run / unresolved

- **No Cargo command was run.** `cargo test`/`cargo clippy` for `ti4-engine` (and the
  `muaat.rs`-affected crates) are still to be run by root. The tests above are unexecuted and their
  expected fixture facts (option ids, affordability, Yssaril's opening fleet) are read from source,
  not observed.
- Not claimed: that the parent package is complete; that `yssarilagent` is complete; any performance
  or parity claim.
- Pre-existing behaviour deliberately not changed here (package non-goal, inherited bool-returning
  leader atomicity): `umbat` returns `true` after the player choice is answered even if the system
  choice is declined or `produce_by_ability_capped` fails mid-way (its `Err` is discarded by
  `let _ =`), so a borrowed use can exhaust Ssruu with nothing produced. This already applied to the
  native use; it now also applies through the copy.
- Shared-code observation, not edited: a borrowed use builds its `DecisionContext` with
  `DecisionSource::FactionAbility("muaatagent")` attributed to the borrower, who does not hold that
  card. `factions::muaat::ask` is the only constructor here and it was not changed. If a future
  decision-source audit requires the actor to hold the named card, the shared borrow path (not this
  module) needs a copy-right source. `tests/decision_delivery_inventory.rs` scans production call
  sites; this package adds no new `Choice`/`ask` site.
- `leader_action` is not given sources, so the offer gate evaluates `agent_targets` with
  `DEFAULT` while the effect uses the live `context.sources` — the same pre-existing asymmetry as
  before this package, unchanged.
