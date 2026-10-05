# BF-SSRUU-ROUND-AGENTS â€” bounded copied combat-round bonuses

## Scope and printed text

This package implements only Ssruu (`yssarilagent`) copies of Sol's Evelyn DeLouis (`solagent`) and Letnev's Viscount Unlenn (`letnevagent`). Each text chooses exactly one unit and grants that unit one extra die for the current combat round. The source card can be Readied or Exhausted, but must remain present and unlocked; only the borrower's Readied Ssruu exhausts, and only after a valid unit is chosen. Native Sol/Letnev leader paths and native extra-die fields remain unchanged.

## Timing and event contract

Root owns registration and producers. A fixed ability roster is built for every seated Yssaril borrower/source seat pair; live eligibility rechecks `hooks_cards::borrowable_agents` against the exact source owner and source leader, plus the source leader's live status. Letnev listens after `COMBAT_ROUND_STARTED`; Sol listens after `GROUND_COMBAT_ROUND_STARTED`. Round producers must supply `system`, `attacker`, `defender`, `round_seq`, and for ground combat `planet`. A ground event is emitted only if `has_round_copy(state, content, "solagent")` is true; otherwise existing ground-combat behavior stays unchanged.

The borrower chooses one exact eligible ship in the named space system or one exact ground force anywhere in the named system, including on a planet outside the current ground battle and owned by a third party. The chooser sees exact `(owner, location, unit type, full-roster ordinal, damage/galvanize signature)` options. The mark stores the actual battle planet separately from the selected unit's planet. The roll reader requires both to match the queried ground battle, so a legal selection outside the battle receives no die in that or another fight. Wrong system, stale round, no eligible units, declined/invalid choice, or lost source rights results in no mark, unit change, or Ssruu exhaustion. A successful choice writes a serialized bounded public round mark under `GameState::faction_marks`; the mark identifies borrower/source, system, both planet locations, unit owner/type/index signature, and `combat_round_seq`. It does not touch the native `extra_die_round`/`extra_die_unit` fields.

## Shared roll seam

The root integration calls `borrowed_round_agents::extra_die_for(state, content, sources, system, planet, player, unit_type, unit_index, round_seq)` for each unit's roll and adds its returned bonus. `unit_index` is the zero-based ordinal in the complete eligible roll roster for that owner and location. The reader reconstructs the same roster with `combat::ships_of` for space or `invasion::is_ground_force_here` for ground, then returns 1 only when the live owner/type/location, damage/galvanize signature, and combat scope all match the mark; otherwise 0. Since model units are interchangeable values with no stable ids, the signature and ordinal prevent the bonus from shifting to a different type, owner, location, or state after removal. The key's round sequence makes old marks inert. `borrowed_round_agents::abilities(state)` is the factory root registers in `reactions::arm`; `has_round_copy(state, content, source_agent)` gates ground-event emission.

## Validation plan

Inline fixtures exercise static roster/live exact-source lock, Ready and Exhausted source status, third-party unit ownership, a copied Sol target on a non-fighting planet, exact battle/target locations, one-unit-only marks, stale target signatures after removal, invalid choice without exhaustion, and wrong type/system/round inertness. Actual `CombatWindow` and committed-planet ground-combat fixtures exercise the registered round producers, copy choice, reader, source status, and exact selected-owner die count. Broader Game retry/producer fixtures and Cargo execution belong to root. No root registration or producer edits are made here.

## Implementation record (2026-10-05)

Added the `factions::borrowed_round_agents` package. It exports the fixed-slot factory `abilities`, exact dynamic source probe `has_round_copy`, and serialized mark reader `extra_die_for`. Its target ordinal follows the complete owner roll roster: `combat::ships_of` for space, and `invasion::is_ground_force_here`-eligible ground forces on each planet for Sol. The reader accepts `ContentStore` and `SourceSet` so it rechecks that same roster at roll time. The unit's damage and galvanize flags are saved as part of the signature. The resolver rechecks source and target after the target choice before inserting one `faction_marks` record and exhausting borrower Ssruu. Inline fixtures exercise live source eligibility (including exhausted and locked sources), third-party owners, a Sol selection outside the actual battle planet, stale target behavior after removal, exact scope reads, and actual space/ground windows with exact added-die assertions.

Root integrated the module with `reactions::arm`, event producers, and both per-unit roll sites while this follow-up was underway. These follow-up fixtures have not been compiled or run under Cargo, per the assigned scope. `rustfmt --edition 2024 --check` and scoped `git diff --check` are repeated after this follow-up; root owns the Cargo coordinator and acceptance tests.


## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.
