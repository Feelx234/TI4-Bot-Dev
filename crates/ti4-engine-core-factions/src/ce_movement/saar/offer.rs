//! saar: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE).
//! Owned by CE only. Empty: every point here asks, and its window owner has not merged the `Ask`/`Opt` family
//! (B7, leaders and component actions; the Arborec's asks also need B3's stages).
//!
//! Points left: `timing_abilities`, `mapped_component_actions`, `perform_component`, `commander_unlocked`, `leader_action`, `use_leader` (Deorbit, Scavenge, Mendosa, Gurno Aggero asks).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
