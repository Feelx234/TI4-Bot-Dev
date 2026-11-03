//! The items of the legacy crate that the glue and the wire types name outside themselves (docs/todo/D2-8-wrapper.md, Appendix A,
//! the 81 ties). Each file here is named after the legacy module the items came from; an item is a COPY (a constant, a pure
//! function over model types), never a re-export, so nothing in this crate names a string rule module.
//!
//! Lane B owns `kinds.rs` and the queries: where lane B provides one of these at its final path, the item here becomes a
//! `pub use` of it. Nothing outside lane A should depend on this module's layout.

pub mod ability_gaps;
pub mod action_cards;
pub mod tactical_live;
pub mod strategy_cards_core;
pub mod production_view;
pub mod exploring_dual;
pub mod core_combat;
pub mod combat_live;
pub mod action_cards_view;
pub mod breakthroughs;
pub mod combat;
pub mod core_adapter;
pub mod draft;
pub mod exploration;
pub mod faction_abilities;
pub mod factions;
pub mod game;
pub mod invasion;
pub mod laws;
pub mod neutral_units;
pub mod planets;
pub mod production;
pub mod production_live;
pub mod promissory;
pub mod reactions_core;
pub mod seating;
pub mod starting_tech;
pub mod strategy;
pub mod strategy_cards;
pub mod tactical;
pub mod technology;
pub mod technology_core;
pub mod timing;
pub mod trade_write;
pub mod transactions;
