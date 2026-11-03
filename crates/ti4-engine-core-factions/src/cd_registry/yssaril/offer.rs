//! yssaril: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty on purpose: the points below are not hooks of this table any more (checked by `silent_census.rs` in `ti4-engine-core-tests`).
//!
//! | point | string function | where the core has it |
//! |---|---|---|
//! | Scheming: choose and discard 1 action card | `action_cards_drawn` | `rules::action_cards::draw` and `driver/windows/status_bookkeeping.rs` (Deepgloom Executable, which allows it, is Thunder's Edge) |
//! | Stall Tactics, Mageon Implants, So Ata, Ssruu, hero | `component_actions`, `perform_component`, `use_leader`, `timing_abilities` | `driver/menu_faction.rs`, `driver/turn/faction.rs`, the leader windows and the specs of the reaction table |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
