//! naalu: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty: every point below asks or needs a window that is not in the core yet.
//!
//! | point | string function | blocked on |
//! |---|---|---|
//! | Gift of Prescience, Foresight, Neuroglaive, Z'eu, The Oracle, Mindsieve use | `timing_abilities` (`gift`, `foresight`, `neuroglaive`, `zeu`, `oracle`) | B7 timing abilities and the reaction window (`Ask::Window` is the only ask family) |
//! | M'aban looking at a hand | `may_view_promissory_hand` | typed `transactions::are_neighbours` (B4); a Q hook once it exists |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
