//! yssaril: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty: every point below asks.
//!
//! | point | string function | blocked on |
//! |---|---|---|
//! | Deepgloom Executable: allow Scheming | `action_card_draw_requested` | B6 draw asks (`Ask` family of the cards window) |
//! | Scheming: choose and discard 1 action card | `action_cards_drawn` | B6 draw asks |
//! | Stall Tactics, Mageon Implants, So Ata, Ssruu, hero, Deepgloom transaction | `component_actions`, `perform_component`, `use_leader`, `timing_abilities` | B7 |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
