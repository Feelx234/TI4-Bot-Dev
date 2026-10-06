# BF Naaz Absolute Synergy survivor choice

Status: implementation updated; acceptance tests and independent root review pending. Do not claim
`naazbt` or `naaz_voltron` in the module ledger on this evidence alone.

## Printed choice semantics

Absolute Synergy says to return three of the four mechs in one system and flip the remaining mech
to the Eidolon Maximum. The player chooses which system and, when needed, which space/planet spot
keeps the survivor. When the selected spot contains mechs with different visible forms, damage, or
galvanize-token states, the resolver now asks which state to keep. Its stable choice ID is
`survivor|<unit-type-id>|<damaged-or-undamaged>|<galvanized-or-plain>`; the resolver maps that
selected state back to its exact board position before removing the other mechs. Identical mech
values have no individual identity in the game model, so they share one option and the first
canonical position represents that equivalent group.

The existing system IDs and `space`/planet spot IDs remain unchanged. A spot with only one distinct
unit state gets no additional question. The chosen unit retains its sustained-damage state when its
type changes to `naaz_voltron`.

## Fixtures added or strengthened

- A faction-resolver fixture selects a damaged ground-form mech among otherwise undamaged
  space-form mechs and asserts the Maximum retains damage.
- The existing same-state four-mech fixture still resolves with only the existing optional and
  spot answers, covering the no-extra-question case.
- The real `Game.step` staged-event retry fixture now fails at the nested survivor-state question,
  asserts board state and decision log rollback, retries, then asserts one outer ability record and
  the selected damaged survivor.

## Verification and claim boundary

No Cargo command was run for this scoped change. Formatting and acceptance checks remain for the
single coordinator. Shared combat and invasion sources now include the Maximum's unit-ability hit
immunity and its participation as a ship in space combat from a planet with a supporting fleet;
the Naaz combat-completion and invasion evidence documents those routes. This review found no
remaining source blocker for those two printed behaviors.

The new Direct Hit route also targets a sustained Maximum returned by `combat::ships_of`, so a
planet-standing Maximum is eligible only while its owner has a ship in the system. The resolver
removes the matching damaged unit from the planet pool and retains the card's direct-hittable
protection check. The fixture includes a supporting cruiser, confirms the planet Maximum is
removed while the cruiser remains, and checks the staged destruction cause and combat context.
Acceptance and the broader `naazbt`/Maximum ledger status remain with root review and validation.
