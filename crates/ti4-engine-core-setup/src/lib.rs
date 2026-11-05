//! Layer L7: the game setup that builds the typed core position (`CoreState`) directly (docs/SETUP_NATIVE_DESIGN.md).
//!
//! `start_game(&TableSpec, Galaxy) -> CoreStart` sets a table up with no string `GameState` and no `Codec::load`. The pure data and
//! random-stream parts of the setup live here and are shared with the thin crate `ti4-engine` (decks, strategy sets, leaders,
//! map feasibility, the live kernel, the unit roster, the typed galaxy). This crate names neither `ti4_engine` nor the legacy crate.

pub mod deck;
pub mod error;
pub mod factions;
pub mod feasibility;
pub mod galaxy;
pub mod ids;
pub mod kernel;
pub mod leaders;
pub mod mecatol;
pub mod rng;
pub mod roster;
pub mod start;
pub mod strategy;

pub use error::SetupError;
pub use start::{CoreStart, SpeakerChoice, TableSpec, start_game};
