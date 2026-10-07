# BF-SSRUU-WINNU-TIMING

## Scope

Support Ssruu borrowing Winnu agent Berekar Berekon's optional `PRODUCTION_USED` ability. A
readied Ssruu holder may use the copied text when the source Winnu agent is readied or exhausted.
The copy reduces the current combined production bill by 2, exhausts only Ssruu, leaves the source
agent unchanged, and notifies `hooks_cards::borrowed_agent_used` after the discount is applied.

Native Winnu use remains unchanged and exhausts only the native agent.

## Implementation and focused coverage

Implementation is in `crates/ti4-engine/src/factions/winnu.rs`. The native and copied options use
separate timing abilities. The copied listener belongs to a specific Ssruu holder and is registered
from the static leader-card roster, independent of current readiness. Its condition checks the
current `borrowable_agents` query at resolution time.

Focused tests added:

- `ssruu_copies_winnu_production_discount_for_readied_or_exhausted_source`: emits a real
  `PRODUCTION_USED` event, declines native use when the source is readied, checks the previewed
  zero-cost cruiser, and places it through `ProductionWindow` with no trade goods spent.
- `borrowed_agent_readiness_is_live_and_registration_uses_the_static_roster`: arms the resolver
  with both cards exhausted, readies both afterward, then confirms the copied ability resolves.
- `borrowed_agent_is_inert_without_ready_ssruu_or_an_available_source`: covers absent/exhausted
  Ssruu, locked or absent source agent, and verifies no source leader is fabricated.

## Validation and review status

No Cargo command or tests were run in this package; the root agent owns shared-build scheduling and
will validate the focused and affected-crate tests. Independent review tier C is pending. No asset
completion claim is made here. No shared files, commits, or generated artifacts were changed.

## Coordinated validation and independent review

Root Codex independently reviewed Luna's source and fixtures: optional ownership is the borrower, exact source lookup is dynamic, static registration survives readiness changes, source status stays unchanged, and the recipient's combined production cost is reduced before the borrowed-use notification. The printed text permits any player's PRODUCTION; no additional event-player restriction is appropriate. No unresolved finding in this narrow package.

Root `cargo test -p ti4-engine -q -j1` passed: **2029 library tests, 0 failures, 1 ignored**, integration groups **1/1, 4/4 decision inventory, 5/5**, doctests passed. All three new Winnu fixtures passed in that suite, including real zero-resource cruiser placement. `cargo clippy -p ti4-engine --lib -q -j1` exited 0 with existing/shared-WIP warnings; no lint-clean claim. The implementer did not run Cargo.

This focused commit stages only copied-listener registration, callback, helper and tests plus this evidence. Earlier Winnu hero/Alliance/BT WIP stays unstaged. Tests ran on the full shared working tree; no isolated committed-tree build is claimed. The broader faction package and milestone remain open.
