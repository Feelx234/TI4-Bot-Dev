//! titans: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty: every point below needs a typed resolution of the unit a player places, or asks.
//!
//! | point | string function | blocked on |
//! |---|---|---|
//! | Hecatoncheires instead of a PDS (offer and perform) | `hecatoncheires_offer`, `hecatoncheires_perform` | typed `action_cards::placed_unit_id` (upgrade, faction unit, `unit_form_override`) and `supply::allowed` (the supply owner / B3); the perform is a no-ask hook and ports with the offer |
//! | Terragenesis, Awaken, Ouranos, Ul the Progenitor, Terraform | `timing_abilities`, `component_actions`, `perform_component`, `use_leader` | B7 |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
