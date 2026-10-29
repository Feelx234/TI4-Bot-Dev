//! L4: rule areas (docs/ENGINE_CORE_STATE.md 4.3). One file per area, owned by its package; an area
//! never imports another area (it returns [`crate::kernel::Next`] instead).

//! # The `Env` traits (kept on purpose)
//!
//! Five areas still ask a trait object for answers the core does not hold: `leaders::Env`, `strategy::Env`,
//! `abilities::Env`, `agenda::Env` and `query::trade::Env`. Each is implemented by the string engine's view of
//! that area (`leaders_view`, `strategy_view`, `agenda_view`, `trade_core`), which calls the string faction
//! modules, because those factions are not yet answered through the kernel hook table (wave C). Where the hook
//! table already answers, the area has an implementation over it (`trade::KernelEnv`, `agenda::window::Facts`).
//! They overlap in four methods (`resource_bonus`, `substitutes_primary`, `secondary_is_free`,
//! `trades_action_cards`); they are not merged because their implementors differ per area and the traits go away
//! wholesale once the factions answer through the kernel. Do not add a sixth.

pub mod abilities;
pub mod action_cards;
pub mod agenda;
pub mod combat;
pub mod diplomacy;
pub mod explore;
pub mod explore_windows;
pub mod invasion;
pub mod leader_windows;
pub mod leaders;
pub mod objective_windows;
pub mod objectives;
pub mod production;
pub mod reactions;
pub mod relic_windows;
pub mod strategy;
pub mod strategy_flow;
pub mod strategy_primaries;
pub mod strategy_windows;
pub mod trade;
