# BF-SSRUU-L1Z1X — copied activation timing

## Scope and permissions

Permission class: P1. Writable paths are `crates/ti4-engine/src/leaders.rs` and this evidence file only. Read-only inputs are the October 4 Pi handover, `plans/SCOPED_PERMISSIONS.md`, seated-agent census, printed leader corpus, and existing activation/borrowable-agent APIs. No network, commits, folder creation, deletion, or external state changes. Cargo checks use the shared coordinator only when the root requests a slot.

## Printed behavior

PoK `l1z1xagent` I48S in `crates/ti4-content/content/leaders.json`: “After a player activates a system: You may exhaust this card to allow that player to replace 1 of their infantry in the active system with 1 mech from their reinforcements.” The typed `SYSTEM_ACTIVATED` event carries `player` (the activating beneficiary) and `system` (the exact active location). Ssruu's owner is the copied text user; the activating player owns the infantry, faction mech identity and reinforcement supply. The source L1Z1X agent may be Readied or Exhausted, and must remain unchanged. Only the borrower's Readied `yssarilagent` exhausts after a successful replacement.

The static timing roster contains one potential slot for each seated Yssaril borrower/L1Z1X source pair. At offer and effect time, live rights are rechecked through `hooks_cards::borrowable_agents` for the exact `(source owner, l1z1xagent)` pair. Legal supply and active actor/location are also recomputed. No infantry or no mech reinforcement means the slot is not offered. Decline, an invalid resolver choice, stale source rights or a mismatched event must leave pieces and leader statuses unchanged. Native I48S dispatch remains unchanged; copied resolution reuses its established conversion so beneficiary, active system, faction mech and supply logic remain consistent, with the borrower passed as text user for promise accounting.

## Shared registry seam

The shared standing ability registry is `crates/ti4-engine/src/reactions.rs::arm`. This package exports `leaders::ssruu_l1z1x_activation_abilities(state)`; root owns and has added the one-time factory registration outside the per-seat loop. The factory is exercised through direct resolver fixtures and a Game-level tactical activation fixture.

## Implementation and validation

Implementation and fixtures are present. Root added a Game activation checkpoint that restores the activation transaction when a reaction fails; its integrated rollback/retry regression and Cargo execution remain pending. Independent tier-C review also remains pending.

## Implementation record (2026-10-05)

Implemented `leaders::ssruu_l1z1x_activation_abilities` in `leaders.rs`. At arm time it produces a fixed slot for each seated Yssaril borrower and L1Z1X source pair, including a borrower whose Ssruu is presently exhausted; each slot is owned by that borrower and optional. Both offer and effect recheck `hooks_cards::borrowable_agents` for the exact source seat and `l1z1xagent`, the event actor/system against the live activation, actor infantry locations, faction mech identity, and `supply::allowed`. The active player selects among eligible planets when there is more than one; an invalid selection propagates before mutation. Success replaces only that actor-owned infantry, records the borrowed use as the Ssruu holder, invokes the shared successful-copy hook, exhausts only borrower Ssruu, and leaves the source status intact. Native `dispatch_leader` and the six native agent implementations are untouched.

The shared registry call was added by root at `reactions.rs::arm`, once outside the per-seat loop. Focused fixtures now cover both Readied and Exhausted L1Z1X sources, active actor versus borrower/source unit ownership, selection of a second legal planet, live source lock after arming, no infantry, a Naaz Maximum placement ban, decline, invalid ability ID, invalid planet ID, and a full `Game` tactical activation reaching `SYSTEM_ACTIVATED` through the registered resolver. Test source is present but was not run by this implementer because the root owns the sole Cargo coordinator. `rustfmt --edition 2024 --check crates/ti4-engine/src/leaders.rs` and `git diff --check -- crates/ti4-engine/src/leaders.rs plans/evidence/BF-SSRUU-L1Z1X-TIMING.md` passed after formatting.

The first root Cargo compile reported the factory's `borrower` moved into the filter closure before use in the mapped closure. This was fixed by cloning a separate `borrower_for_filter`. No post-fix Cargo result is available to this implementer; compilation and focused test status remain pending root's coordinated rerun.

Root has since added an activation checkpoint in `Game::step_tactical` that restores the activation state, dice, RNG, event sequence, resolver, external table log, events, pending tactical data, and galaxy when applying the activation reaction fails. Root owns the integrated Game rollback/retry fixture and coordinator rerun; neither result is claimed here. The leaders fixtures independently verify that invalid target selection returns before piece or leader mutation.

Independent tier-C review remains pending. No commit, Cargo run, or shared `reactions.rs` edit was made by this implementer.

## October 5 root source review and runtime results

Root reviewed the Luna factory/readiness/ownership/supply and actual tactical-activation path. Fixture setup previously incorrectly derived System planets from Planet.system_id; repaired to actual System.planets(). All L1Z fixtures and the real Game invalid nested planet/same-Table retry passed in the full lib2088/0/1 run. Activation snapshots include state, dice/RNG, sequence/resolver, external log, events, tactical, galaxy, turn-closing and action bookkeeping. Naaz beneficiary placements now additionally stage the scoped fourth-mech event. Final full-engine/inventory/lint and isolated-hunk commit remain pending.
