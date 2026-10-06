# BF-SSRUU-SAAR-TIMING

## Scoped implementation authorization (P1)

This evidence file records the task's P1 authorization before implementation. Writable scope is limited to `crates/ti4-engine/src/factions/saar.rs` and this evidence file. No production/shared files, Cargo commands, commits, new folders, deletions, or subagents are authorized.

## Corpus rule and implementation target

Saar's Captain Mendosa (`saaragent`) says: “When a player activates a system: You may exhaust this card to increase the move value of 1 of that player's ships to match the move value of the ship on the game board that has the highest move value.” The existing Saar-native ability is already registered and stores its activation-scoped movement bonus. Add the same text as an Ssruu copy: the borrower controls the optional activation listener and selection, Ssruu is the only card exhausted, and the source Saar agent remains at its existing Readied or Exhausted status. Borrowability must be queried dynamically through the shared `borrowable_agents` contract, including the other-seat and readied-Ssruu constraints. The selected ship's bonus must remain tied to the activating player's actual activation, origin and ship index so it is consumed by movement at the correct time and does not leak to another ship/activation.

## Acceptance coverage to add

Add inline tests in Saar's existing test module that use an actual `SYSTEM_ACTIVATED` timing window/Resolver path and verify borrowed selection, Ssruu exhaustion, source-agent status preservation, the computed movement bonus at the selected ship's actual move site, no bonus for another ship/activation, unavailable-source/readiness gating, and the delayed activation-scoped mark. No tests are run in this supplemental task; root coordinates validation.

## Implementation and coverage status

Implemented in `factions/saar.rs`: for each seated Saar source, a static `SYSTEM_ACTIVATED` listener is armed per other seated borrower. Its condition and resolver callback both query `hooks_cards::borrowable_agents` for the exact source `saaragent`, so a copied listener becomes live when Ssruu is gained/readied and stops when the source disappears or Ssruu is exhausted. An accepted choice exhausts only borrower `yssarilagent`, writes the existing activation/origin/ship-index scoped mark, and calls `borrowed_agent_used`; it does not mutate source status. The native Saar listener and movement hook remain intact.

Added tests: `ssruu_borrows_captain_mendosa_on_the_activation_and_boosts_only_the_selected_ship` drives the real armed `SYSTEM_ACTIVATED` Resolver and verifies the selected move site's bonus, another ship's lack of bonus, expiry at the next activation, Ssruu exhaustion, and an already-exhausted source staying exhausted. `ssruu_captain_mendosa_listener_tracks_source_and_borrower_readiness_live` checks that a readied then exhausted Saar source remains copyable, and drives actual windows proving source removal and exhausted Ssruu suppress the copied effect.

Tests were not run in this supplemental task; root owns validation. The movement bonus assertion reads the engine's `MoveSite` hook at the selected ship's site after the real activation event; it does not itself run a full tactical move through the `Game` command path.

## Fixture correction after root validation

The exhausted-Ssruu activation also had a separately eligible native Saar `saaragent` because the fixture re-readied the source. Its final `SYSTEM_ACTIVATED` window now explicitly answers `decline` for that native optional branch, keeping the assertion focused on the copied listener's exhausted-Ssruu gate while preserving native behavior. Root reported this fixture as the sole Saar failure in its full library run; a post-fix retry is pending.
