# BF-SSRUU-GHOST-TIMING

## Package specification and scope

Implement Ssruu copying the Ghost agent Emissary Taivra at its printed `SYSTEM_ACTIVATED` timing
window. Its copied listener is borrower-owned and separately optional from the native agent;
listener registration uses the static source/borrower leader roster, while the live condition checks
the current `borrowable_agents` result. The source agent may be Readied or Exhausted. A copied use
exhausts only the borrower's Ssruu, leaves the source unchanged, and notifies the borrowed-agent
hook after the wormhole link effect succeeds.

The activation must satisfy the same printed restrictions as native use: the activated system has
an alpha or beta wormhole (delta alone does not qualify), and there is at least one other system
containing a wormhole. The copied link applies to the activated system for the current tactical
action and is keyed to the activation sequence as the native link is.

Writable paths are limited to `crates/ti4-engine/src/factions/ghost.rs` and this evidence file.
No Cargo runs, shared-file edits, commits, new directories, or generated artifacts are authorized
for this package. Root owns build, focused/crate validation, and independent tier-C review.

## Implementation and focused coverage

Implemented a separate optional `SYSTEM_ACTIVATED` ability controlled by the Ssruu holder. Its
registration comes from the static seated-leader roster; its condition rechecks live borrowability
and the native wormhole restrictions. Successful use exhausts Ssruu, leaves the source card alone,
sets the existing activation-scoped adjacency link, and invokes the borrowed-agent hook.

Focused regression tests added:

- `ssruu_copies_emissary_taivra_for_readied_or_exhausted_source`
- `borrowed_agent_readiness_is_live_after_resolver_registration`
- `copied_agent_rejects_unavailable_cards_and_illegal_wormhole_targets`

The tests cover source Readied/Exhausted, dynamic readiness after resolver registration, missing or
unavailable cards, delta-only and non-wormhole activations, and the absence of another wormhole
system. They also assert that Ssruu owns the copied optional decision, only Ssruu exhausts, and the
source status is unchanged.

## Validation and review status

No Cargo test or build was run, as required by the package boundary. Source formatting and whitespace
checks are run before handoff. Root validation and independent tier-C review are pending. No asset
completion claim is made.

## Root independent review and validation

Root Codex reviewed Luna's actual source, static listener roster and dynamic exact-source eligibility, borrower-owned optional prompt, source-status preservation, activation-scoped link, and target legality. Native readiness remains gated independently; both paths share the same wormhole target predicate. No new unresolved finding in this narrow package. Source Readied/Exhausted, absent/locked cards, readiness changes and illegal wormhole targets are covered by the new fixtures.

Coordinated `cargo test -p ti4-engine -q -j1`: **2029 library tests passed, 0 failed, 1 ignored**; integrations **1/1, 4/4 decision inventory, 5/5**, doctests passed. All three Ghost copy fixtures passed. Root repaired a test-only BTreeMap construction to own its string keys before the passing run. `cargo clippy -p ti4-engine --lib -q -j1` exited 0 with existing/shared-WIP warnings; no lint-clean claim. Luna did not run Cargo.

Focused staging includes only copied activation behavior and its helpers/tests/evidence. Earlier Ghost commander/Alliance WIP is preserved unstaged. Tests ran against the full shared tree; no isolated committed-tree build is claimed. This package does not close the whole faction plan.
