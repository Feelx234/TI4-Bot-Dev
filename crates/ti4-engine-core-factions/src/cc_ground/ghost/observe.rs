//! ghost: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Nothing to port: the string module has no `Obs` or bookkeeping hook (the agent link and the commander count are written by
//! its abilities, tier W).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
