# BF-SSRUU-NAAZ-TIMING

## Scope

Extend Naaz Garv and Gunn (`naazagent`) at its existing `TURN_PASSED` timing window so a player
with readied Ssruu can copy the agent text when the Naaz source agent is readied or exhausted.
The Ssruu holder controls whether to use the copy; the ending player selects the planet. Exploration is credited to
the player whose turn ended. A copied use exhausts only Ssruu, leaves the source status unchanged,
and notifies `hooks_cards::borrowed_agent_used`.

Native use remains gated on the Naaz agent being readied. It retains the prior ending-player planet
selection and exhausts the native agent.

## Implementation and focused coverage

Implementation is in `crates/ti4-engine/src/factions/naaz.rs`. The native Naaz timing ability is
unchanged and remains owned by the source player. A separate optional `TURN_PASSED` listener is
registered per static source/borrower leader roster; its live condition checks `borrowable_agents`,
so readiness changes do not depend on rebuilding the resolver. The copied optional window belongs
to the Ssruu holder. After that player accepts, the ending-turn player chooses a planet; exploration
is credited to that player. Only the borrower's `yssarilagent` exhausts, the source status is
unchanged, and the borrowed-agent-used hook runs after exploration.

Focused tests added:

- `ssruu_copies_garv_and_gunn_for_the_ending_players_planet` (covers source agent Readied and Exhausted)
- `copied_agent_is_inert_without_readied_ssruu`
- `copied_agent_is_inert_when_the_source_agent_is_absent`
- `breakthrough_gain_window_can_flip_four_mechs_into_the_maximum`
- `production_choices_hide_mechs_only_while_the_maximum_stands`

Existing native tests continue to cover ready-agent exploration, decline, and the requirement that
the native agent be readied.

## Validation and review status

No Cargo command or tests were run in this package; the root agent owns shared-build scheduling and
will validate the focused and affected-crate tests. Independent review tier C is pending. No asset
completion claim is made here. No shared files, commits, or generated artifacts were changed.

## Root review and validation

Root rejected the initial source-owned copied prompt and borrower-controlled target choice; the final separate listener is borrower-owned and the ending player chooses the planet. Root additionally replaced the copied planet-selection helper (which swallowed errors) with a strict observed Choice and registered `naaz.rs::borrowed_agent` in both decision inventory lists. Native legacy choice behavior is preserved. `copied_naaz_planet_choice_errors_before_exhaustion_and_can_retry` passed **1/1**, 2022 filtered: invalid target returns TimingError::IllegalChoice before state mutation; a fresh emission successfully retries and exhausts only Ssruu. This tests pre-mutation refusal, not automatic resolver-journal rollback.

The preceding coordinated `ssruu` filter passed **14/14**, 2004 filtered, before this additional test and the latest Winnu patch. Full affected-crate checks remain outstanding. Nested exploration still exposes legacy Option-returning APIs and swallows nested choice errors; this correction does not claim to propagate those errors. Independent review of the root strict-choice correction remains pending.

First full library run found the new production-offer fixture lacked a controlled ground destination in system 18: it could not offer a mech even after removing the Maximum. Root explicitly assigns Mecatol control in this fixture so the negative/positive pair measures the Maximum restriction rather than ordinary placement legality. Focused retry pending.
