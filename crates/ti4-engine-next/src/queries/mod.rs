//! The native queries of the product surface (docs/todo/D2-8-wrapper.md, lane B / 8d): one file per legacy module, the same function names and
//! signatures, answers read from the typed core. `tests/surface_parity.rs` compares each with its legacy original.

pub mod adapter;
pub mod tactical;
pub mod movement;
pub mod reactions;
pub mod laws;
pub mod production;
pub mod view;
pub mod factions;
pub mod fracture;
pub mod diplomacy;
pub mod transit;
pub mod fleet;
pub mod content;
pub mod fleet_strength;
pub mod technology;
pub mod objectives;
pub mod secrets;
pub mod cards;
pub mod planets;
pub mod supply;
pub mod combat;
pub mod invasion;
pub mod legendary;
pub mod entropic_scars;
pub mod promissory;
pub mod strategy_cards;
pub mod valuation;
pub mod action_cards;
pub mod faction_abilities;
pub mod strategy;
pub mod secondary_preview;
pub mod reach_query;
pub mod timing;
