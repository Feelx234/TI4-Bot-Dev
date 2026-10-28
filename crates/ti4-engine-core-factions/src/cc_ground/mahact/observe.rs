//! mahact: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Nothing to port: the string module has no `control_gained` or other observer (the fleet pool is written by `add_token` /
//! `remove_token`, called from the abilities and the Starlancer, tier W).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
