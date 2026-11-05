//! Mecatol Rex, either printing (pure: tile ids and a galaxy).

use ti4_content::ContentStore;
use ti4_content::galaxy::{Galaxy, all_systems};
use ti4_model::content_types::SourceSet;

/// Mecatol Rex, which sits at the centre of the board: the base tile.
pub const MECATOL: &str = "18";

/// The Thunder's Edge Mecatol Rex tile, whose planet (`mrte`) is legendary (The Galactic Council).
/// Placed instead of [`MECATOL`] whenever Thunder's Edge content is in scope.
pub const MECATOL_TE: &str = "112";

/// Whether `system` is Mecatol Rex, either printing.
#[must_use]
pub fn is_mecatol(system: &str) -> bool {
    system == MECATOL || system == MECATOL_TE
}

/// Whether `planet` is the planet Mecatol Rex, either printing.
#[must_use]
pub fn is_mecatol_planet(planet: &str) -> bool {
    planet == "mr" || planet == "mrte"
}

/// The Mecatol Rex tile a new board uses under `sources`: the Thunder's Edge printing when it is
/// available, else the base tile.
#[must_use]
pub fn mecatol_for(content: &ContentStore, sources: SourceSet) -> &'static str {
    if all_systems(content, sources).contains_key(MECATOL_TE) {
        MECATOL_TE
    } else {
        MECATOL
    }
}

/// The Mecatol Rex tile placed on this map (the base tile when neither is placed).
#[must_use]
pub fn mecatol_in_galaxy(galaxy: &Galaxy) -> &'static str {
    if galaxy.coord_of(MECATOL_TE).is_some() {
        MECATOL_TE
    } else {
        MECATOL
    }
}
