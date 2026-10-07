# BF Alliance commander routes: C

This package makes the named commander ability follow the current recipient's rights, including a
faceup Alliance from an owner with that exact commander unlocked and direct public grants. Native
commander unlock predicates remain unchanged.

- Naalu M'aban privacy now checks `has_commander_ability` on the viewing seat, then checks the
  live map adjacency to the hand owner. The typed agenda-deck reader uses the same commander-rights
  predicate through `commander_has_peek`; it remains bound to the observation's acting seat.
- Sardakk G'hom Sek'kus commit candidates now honor a recipient's borrowed commander ability, so a
  recipient of any faction can commit its own force from the active system and eligible adjacent
  systems.
- Yin Brother Omar's green prerequisite and owned-elsewhere technology waiver now use the same
  rights predicate. The Yin unlock condition (use a faction ability) remains native and unchanged.
- Yssaril So Ata's event listener already registers for every seat; its live activation predicate
  now uses the recipient's commander rights and continues to reveal only to that recipient.

Coverage added for direct ownerless grants, ordinary Alliance rights while unlocked, privacy while
locked, recipient and neighbor boundaries, borrowed Sardakk landing, Yin's prerequisite and waiver,
and Yssaril's recipient-bound reveal. The evidence review found one cleanup seam: So Ata's
`ACTION_COMPLETED` cleanup still gates on a native leader record. A tool review rejected removing
that recipient-specific guard because that would broaden cleanup to unrelated seats. This package
leaves the guard unchanged; cleanup behavior for an Alliance-only recipient needs a recipient-bound
implementation.

Focused test names:

- `factions::naalu::tests::alliance_maban_rights_are_live_neighbor_scoped_and_recipient_bound`
- `factions::naalu::tests::ownerless_commander_grant_enables_neighbor_hand_and_agenda_peeks_for_recipient`
- `factions::sardakk::tests::an_ownerless_alliance_grant_allows_any_faction_to_commit_its_neighbor_force`
- `factions::yin::tests::brother_omar_prerequisite_and_waiver_follow_alliance_rights_for_any_faction`
- `factions::yssaril::tests::ownerless_alliance_recipient_gets_only_its_so_ata_activation_reveal`

Tests were not run in this bounded package because the coordinator owns Cargo builds for the shared
working tree.

## Root resolution and validation

The rejected blanket So Ata cleanup removal was not retried. Root instead bound cleanup eligibility to the actual recipient reveal row (`cards:reveal:<scope>|yssarilcommander|<viewer>|...`); the ownerless-recipient fixture now verifies ACTION_COMPLETED clears that reveal. Naalu privacy fixtures isolate all presence/control and put the recipient on the outer ring, owner at centre, unrelated player opposite. The owner's own unlocked commander remains valid; recipient binding is checked on the unrelated seat. Sardakk's fixture removes default ground forces before placing its single known neighbor candidate. All these routes pass the 1,986-pass engine library and full inventory/integration run. Later tests are separately recorded.
