//! ghost: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Empty until the windows exist.
//!
//! Not ported (window owners), with the reason:
//!
//! * `combat.produced_hits` (the Ghost mech/flagship rolls), `timing_abilities` (Emissary Taivra, the commander),
//!   `commander_unlocked`, `leader_action` / `use_leader` (the hero swap), `component_actions` / `perform_component`
//!   (the Creuss gate token, Icarus Drive, the Wormhole Generator): B1 and B7.

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
