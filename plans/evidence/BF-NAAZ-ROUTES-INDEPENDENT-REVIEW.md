# Supplemental Naaz route review

Reviewer: Luna (independent implementation context; supplemental review only, not frontier tier-C
acceptance).

## Scope

Read-only review of `factions/naaz.rs::borrowed_agent`,
`copied_naaz_planet_choice_errors_before_exhaustion_and_can_retry`, the corresponding producer and
observed-ask row in `decision_delivery_inventory.rs`,
`game.rs::a_planetary_maximum_is_immune_to_the_actual_space_cannon_offense_route`, and the actual
invasion-window space-cannon, bombardment, and Harrow fixtures.

## Findings

No blocking correctness finding in the reviewed changes.

The copied Naaz listener owns the optional timing use, but builds its nested planet choice for the
`TURN_PASSED` event's player. It asks before exhausting Ssruu or calling `explore_planet`; an invalid
planet id therefore returns `TimingError::IllegalChoice` before this callback mutates state. The
test compares the entire `GameState` to its pre-event clone after that error. Its subsequent retry
uses the `emit` helper, which constructs a fresh resolver. This demonstrates that the effect can be
retried from unchanged game state; it does not claim that resolver journal, event sequence, or
decider state were rolled back for a retry on the same resolver.

The ordinary copied-use fixture also verifies that the timing offer is owned by Ssruu while the
planet choice is owned by the ending player. The callback carries that actor through to
`explore_planet` and the resulting exploration record. The inventory registers exactly one observed
ask at `naaz.rs::borrowed_agent`, matching its single `ask_seeing` call.

The tactical cannon fixture reaches `SPACE_CANNON_HITS` through repeated `Game::step` calls, checks
two PDS dice and two hits, asserts both that no `naaz_voltron` casualty/sustain option was offered
and that the attacking cruiser was removed while the Maximum remained intact, then checks that the
game stopped in tactical aftermath before space combat. The combat hit-assignment path includes the
planetary Maximum when it participates in space combat, so the immunity assertion exercises a
candidate the route can actually offer.

The invasion fixtures drive the relevant windows and check unit balances directly: bombardment
removes the only eligible infantry while the Maximum remains, space-cannon defense removes the
committed infantry while the Maximum remains, and Harrow removes the eligible infantry while
preserving the Maximum. Their deterministic dice and final-unit assertions substantiate the
intended route outcomes; the no-sustain-prompt assertions are supporting checks rather than the sole
evidence.

## Validation

Review was read-only. No Cargo command was run. This supplemental pass is not a tier-C frontier
acceptance.
