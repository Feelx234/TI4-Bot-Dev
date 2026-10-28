//! borrowed: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Nothing to port: the helpers have no observer; `borrowed_agent_used` / `borrowed_agent_used_state` are `cards.rs` hooks of the
//! lending modules (package CD, Yssaril).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
