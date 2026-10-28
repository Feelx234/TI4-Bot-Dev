//! mahact: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only. Empty until the windows exist.
//!
//! Not ported (window owners), with the reason:
//!
//! * `strategy.secondary_waived` (Jae Mir Kan: exhaust the agent, take the token off the board): needs the kernel's leader
//!   exhaust, which also tells the Nomad's Suite (`Obs`-like event, package CE).
//! * `mahact_units_offer_starlancer` (asks each eligible holder, then `strategy_cards::gain_tokens`): B-window `Ask` family.
//! * `timing_abilities` (Edict, Imperia, Scepter, the Legionnaires), `commander_unlocked`, `component_actions`,
//!   `perform_component`, `leader_action`, `use_leader_timed` (Airo Shir Aur): B7.
//! * `add_token` / `remove_token` / `store` (the pool writers) come with the abilities that call them.

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
