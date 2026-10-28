//! firmament: tier O: observers (`Obs`) and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Nothing to port as an observer: the plot scoring bookkeeping (`plot_objectives`, the Sowing goods, the plot flips) is written
//! by `place_card` / `score_as_plot` / `flip_plots` and the abilities, which are tier W (`offer.rs`).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
