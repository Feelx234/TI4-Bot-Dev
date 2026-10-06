# BF-SSRUU-ARGENT-TIMING

## Package scope

Permission class: P1. Writable paths: `crates/ti4-engine/src/factions/argent.rs` and
`plans/evidence/BF-SSRUU-ARGENT-TIMING.md`. Read-only sources include the repository instructions,
the Argent agent record in `crates/ti4-content/content/leaders.json`, and existing borrowed-agent
patterns in Naalu and Winnu. Network, Cargo, commits, folder creation, and edits to shared files are
out of scope.

Objective: let a player with Readied Ssruu copy a seated Argent agent's printed production timing
effect. Registration uses the static source/copy card roster; dynamic eligibility requires the
exact source agent to be borrowable now. A successful copied use exhausts only Ssruu, preserves the
source agent's state, grants the production destination mark to the producing player, and notifies
the borrowed-agent hook. Native Argent behavior remains unchanged.

## Implementation and evidence

The native listener and production destination hook were preserved. The copied listener is
registered from the static Argent-agent/Ssruu leader-record roster, while its stateful condition
checks the exact Argent source through `hooks_cards::borrowable_agents`, live Readied Ssruu status,
the producer's positive unit count, legal controlled planets, and the production system. On use it
exhausts Ssruu only, stores the same recipient/system destination mark consumed by the production
window, and calls `hooks_cards::borrowed_agent_used`. It retains the event's producing player as the
effect target.

Focused tests added in `crates/ti4-engine/src/factions/argent.rs`:

- `ssruu_uses_argent_agent_on_a_real_infantry_production_window`
- `copied_agent_readiness_is_live_after_resolver_arming`

`rustfmt --edition 2024 crates/ti4-engine/src/factions/argent.rs` and
`git diff --check -- crates/ti4-engine/src/factions/argent.rs plans/evidence/BF-SSRUU-ARGENT-TIMING.md`
passed. Cargo was not run locally. Focused and engine tests plus tier-C review remain for the root
coordinator.
