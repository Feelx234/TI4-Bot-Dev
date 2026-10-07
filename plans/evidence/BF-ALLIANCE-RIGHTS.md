# BF-ALLIANCE-RIGHTS: shared commander ability foundation

Status: foundation implemented; downstream faction-effect dispatch and Yin BT trigger/random selection are not included in this package.

## Rule basis

The source corpus entry `crates/ti4-content/content/breakthroughs.json` (`yinbt`, Thunder's Edge) says: “When you gain this card or score a public objective, gain the alliance ability of a random, unused faction.” The foundation stores that acquired ability as a public recipient-specific fact without changing the absent faction's leader record.

Ordinary Alliance remains distinct. Promissory notes are faceup in play areas under LRR 69.3, and the existing note availability / `commander_ability_from` routes continue to require an ordinary seated owner whose commander is unlocked. The new shared predicate recognizes that ordinary route only for the exact requested commander and only while the Alliance note is faceup and held by the recipient.

## Implementation

`promissory::has_commander_ability(state, player, commander)` returns true for the player's exact unlocked commander, an ordinary faceup Alliance held by that player whose seated owner has that exact commander unlocked, or a direct public grant mark. It does not treat a facedown note, foreign-held note, absent ordinary Alliance owner, or another unlocked commander as a match.

`promissory::grant_commander_ability(state, content, player, commander)` validates that the recipient exists and the content record exists with `type == commander`. It inserts `commander_ability:<player>:<commander> = granted` into `GameState::faction_marks`. The prefix is public under `ti4_model::view::mark_visible_to`, and the mark survives the normal serialized `GameState` round trip. A repeated grant returns false without overwriting the existing mark. It does not require a seated owner or unlocked leader and does not mutate any other player's leader status.

No random candidate selection, Yin trigger, or faction-specific ability consumer was added. Existing effect routes still need to call the shared predicate before a direct grant has functional effect.

## Focused coverage

Tests added in `promissory.rs` cover exact own commander matching, ordinary Alliance locked/unlocked/facedown/foreign-held cases, an ownerless direct grant, grant validation and idempotence, unchanged borrowed leader records, public mark visibility, and state serialization persistence. Per parent coordination, Cargo tests were not run; the parent is coordinating the shared build. `rustfmt --edition 2024 crates/ti4-engine/src/promissory.rs` and `git diff --check -- crates/ti4-engine/src/promissory.rs` completed successfully.
