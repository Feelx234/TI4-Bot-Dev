//! The product-facing engine crate. Facade stage: everything not defined here is re-exported from
//! `ti4-engine-legacy` (the directory `crates/ti4-engine`, docs/todo/D2-7-legacy-split.md); each
//! module below shadows the glob and is native (docs/todo/D2-8-wrapper.md section 1.4).
//!
//! One section per lane (section 6.2 of the D2-8 document): a lane edits only its own section.

// ---- lane A (foundation: wire types, glue, Observed) ----

// ---- lane B (surface: kinds, queries, previews) ----

// ---- lane C (native setup and position builders, D2-8c; docs/D28C_RESUME.md) ----
pub mod builders;
pub mod deck;
pub mod fixtures;
pub mod map_feasibility;
pub mod opening;
pub mod seating;
pub mod setup;
pub mod starting_tech;

pub use deck::{EXPLORATION_TRAITS, StartingDecks, build_starting_decks};
pub use seating::{SeatingError, build_board, deploy, home_systems, neutral_systems};
pub use setup::{
    SetupError, cards_per_player, seeded_speaker, start_game, start_game_seeded,
    start_game_seeded_with_card_set, strategy_card_setup, strategy_card_setup_for_set,
};

// ---- lane D (the Game wrapper) ----

// ---- the facade: everything else is still the legacy engine's ----
pub use ti4_engine_legacy::*;
