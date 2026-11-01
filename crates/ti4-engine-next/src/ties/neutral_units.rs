//! From `neutral_units.rs` of the legacy crate (copies).

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::neutral_units::{NEUTRAL};

use ti4_model::id::PlayerId;


/// Whether this owner is the neutral force rather than a seated player.
#[must_use]
pub fn is_neutral(player: &PlayerId) -> bool {
    player.as_str() == NEUTRAL
}
