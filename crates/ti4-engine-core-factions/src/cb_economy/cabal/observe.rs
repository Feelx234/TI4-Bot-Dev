//! cabal: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB).
//! Owned by CB only. Empty on purpose: Cabal's bookkeeping is `silent::cabal` (it stages COMMODITIES_REPLENISHED itself); the slot `direct.cabal_note_replenished` is the dual's legacy hook (`silent_census.rs`).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
