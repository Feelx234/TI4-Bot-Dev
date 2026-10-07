# BF-SSRUU-NAALU-TIMING

Implemented the Naalu Thunder's Edge agent timing copy through Ssruu (`yssarilagent`). The printed
source in `crates/ti4-content/content/leaders.json` identifies the agent as resolving after any
player's command token is placed in a system and returning that token to its owner's reinforcements.
The same card record says Ssruu copies another player's agent text even while that agent is
exhausted.

The existing native Naalu timing listener remains intact. Naalu registration adds a distinct,
borrower-bound optional listener from the static source/copy card roster, so it remains available
when Ssruu readies after resolver construction. Its live condition and effect recheck
`hooks_cards::borrowable_agents`, validate the event target token in the named system, exhaust only
Ssruu, remove the placed token, and notify
`hooks_cards::borrowed_agent_used`. The source agent's Readied or Exhausted state is untouched.

Focused tests added in `crates/ti4-engine/src/factions/naalu.rs`:

- `ssruu_copies_readied_or_exhausted_naalu_agent_and_exhausts_only_ssruu`
- `borrowed_naalu_agent_is_inert_without_readied_ssruu`
- `armed_listener_survives_ssruu_readiness_changing_without_rearming`

Permission scope for this work was limited to `crates/ti4-engine/src/factions/naalu.rs` and this
evidence file. No inventory row was needed because the callback uses the existing timing choice
delivery. No Cargo command was run locally; focused tests and tier-C review remain for the root
coordinator.
