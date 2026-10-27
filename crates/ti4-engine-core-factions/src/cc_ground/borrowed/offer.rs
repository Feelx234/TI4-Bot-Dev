//! borrowed: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Empty until the windows exist.
//!
//! Not ported (window owners), with the reason:
//!
//! * `borrowed_round_agents_extra_die_for`: the copied bonus is a JSON blob mark and `live_target` needs
//!   `invasion::is_ground_force_here` (B2); needs F5's typed record for `YssarilSsruuRoundAgent`.
//! * `borrowed_round_agents_has_round_copy`: needs `hooks_cards::borrowable_agents` (package CD, Yssaril).
//! * `borrowed_round_agents_abilities` and the `timing_abilities` of `borrowed_commanders(_b)`: reaction windows (B1/B7).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
