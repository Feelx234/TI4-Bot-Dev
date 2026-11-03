//! From `neutral_units.rs` of the legacy crate (copies).

use ti4_model::id::PlayerId;

/// The owner id carried by every neutral unit.
///
/// A real `PlayerId` so ownership comparisons work unchanged (rule 9), never seated so nothing
/// iterating players finds it (rule 6). No faction may use this id.
pub const NEUTRAL: &str = "neutral";

/// Whether this owner is the neutral force rather than a seated player.
#[must_use]
pub fn is_neutral(player: &PlayerId) -> bool {
    player.as_str() == NEUTRAL
}
