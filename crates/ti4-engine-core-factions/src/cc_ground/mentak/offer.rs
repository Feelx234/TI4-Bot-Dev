//! mentak: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Empty until the windows exist.
//!
//! Not ported (window owners), with the reason:
//!
//! * `combat.produced_hits` (Mentak's Ambush: asks which ships fire): B1's `Stage` for the produced-hits step.
//! * `timing_abilities` (Pillage, The Table's Grace gain, Ambush), `commander_unlocked`, `component_actions`,
//!   `perform_component`: reaction windows (B1/B7) and the component actions of B7.

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
