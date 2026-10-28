//! naaz: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty: every point below asks, and the `Ask`/`Opt` families of its window are still empty enums
//! (`kernel/choice/*`), a request to the area owner, not a CD edit.
//!
//! | point | string function | blocked on |
//! |---|---|---|
//! | Supercharge at the start of a combat round | `naaz::offer_supercharge` (from `space/ground_combat_round_started`) | B1 `Stage::Offering` + `Ask` for the combat window; ports with the round-start observers (`observe.rs`) |
//! | Garv and Gunn, Hesh and Prit, Dart and Tai, Visz El Vir synergy asks | `component_actions`, `perform_component`, `leader_action`, `use_leader`, `timing_abilities`, `commander` | B7 (leaders, component actions, timing abilities) |
//! | Absolute Synergy flip and the survivor choice | `flip_synergy`, `survivor_choices` | B7 + the events window |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
