//! mentak: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Nothing to port: the string module has no `Obs` or bookkeeping hook (`BREAKTHROUGH_GAINED` is announced through its timing
//! abilities, tier W).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
