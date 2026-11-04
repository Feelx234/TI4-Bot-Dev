//! naalu: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty on purpose: the points below are not hooks of this table any more (checked by `silent_census.rs` in `ti4-engine-core-tests`).
//!
//! | point | string function | where the core has it |
//! |---|---|---|
//! | Gift of Prescience, Foresight, Neuroglaive, Z'eu, The Oracle (Mindsieve is Thunder's Edge) use | `timing_abilities` (`gift`, `foresight`, `neuroglaive`, `zeu`, `oracle`) | specs of the reaction table (`silent::*`, `AbilityTable::specs`) and the leader windows |
//! | M'aban looking at a hand | `may_view_promissory_hand` | the transaction reach in `query::trade` |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
