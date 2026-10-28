//! sardakk: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//! Owned by CD only. Empty: every point below asks.
//!
//! | point | string function | blocked on |
//! |---|---|---|
//! | Sh'val: skip to the commit step | `skip_to_commit` (asks, then writes the hero mark) | `Ask::Leaders` family (B7) and the invasion window's `skip_to_commit` point (B2) |
//! | Exotrireme II, Tekklar Legion, Mech hit, agent, Supremacy, hero finish | `timing_abilities` | B7 timing abilities |
//! | the commander unlock and the leader actions | `commander_unlocked` is Q (`query.rs`); `leader_action`, `use_leader` | B7 |

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
