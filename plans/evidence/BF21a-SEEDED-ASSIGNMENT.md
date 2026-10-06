# BF21a — Seeded faction assignment primitive

## Scope and boundary

BF21a adds a standalone, deterministic helper in `ti4-engine::seating` that accepts an explicit ordered candidate roster, ordered player list, and seed, then returns a unique player-to-faction map or a validation error. It does not widen `IN_SCOPE_FACTIONS`, change `seat_in_scope`, modify any runtime caller, or claim that games now use the helper. BF20 acceptance and BF21b consumer wiring remain separate gates.

## Assignment contract

- Validate the ordered inputs before assignment: reject an empty candidate roster, duplicate candidate aliases, duplicate player IDs, and fewer candidates than players.
- Shuffle the candidate roster using the existing `GameRng` API on a dedicated faction-selection domain, independent from map generation and all current game streams.
- Assign the first `players.len()` shuffled candidates to players in the caller's input order. Never wrap or reuse a faction.
- The same explicit inputs and seed return the same map; different seeds can select a different assignment. Candidate roster order is meaningful and explicit.
- Test the 18 planned aliases as corpus-backed content without changing the current six-faction runtime scope.

## Verification status

No Cargo command or runtime game integration is part of this preparation slice. Source formatting and whitespace checks may be run after the edit; unit tests remain unexecuted here.


Independent root source review: dedicated local GameRng leaves map/runtime streams untouched; inputvalidation precedes map construction and no modulo reuse exists. Focused seating suite41passed,0failed,2067filtered (target/bf-oct5-seating-helper.log). Root replaced a same-shuffle-derived expectation with actual player-order and two-candidate roster-reversal properties, avoiding an implementation-mirroring oracle. Follow-up test/fullgate/lint pending. P1scope source/evidence only; no roster widening, callers, network, processes, folders/deletion or external effect.
