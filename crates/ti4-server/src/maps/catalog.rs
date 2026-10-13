//! What the host can choose, and what each choice looks like. The catalogue, the previews and
//! the description of a choice are in `ti4-view` (`maps::catalog`), where the wasm host has them
//! too; what is of this process is here.

use std::sync::OnceLock;

pub use ti4_view::maps::catalog::{
    MapChoice, MapChoiceView, MapPreview, SeatPreview, catalog, describe, preview, recommended_for,
    scenario_preview, seat_place_id, validate,
};

/// A repeatable seed for the picture of variant `variant` of a card. Mixed with a per-process
/// secret so the numbers cannot be walked to learn anything about a real game's seed.
#[must_use]
pub fn variant_seed(choice: &MapChoice, player_count: usize, variant: u32) -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    let salt = *SALT.get_or_init(rand::random);
    let mut x = salt ^ u64::from(variant).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= player_count as u64;
    if let MapChoice::Template { alias } = choice {
        for b in alias.bytes() {
            x = (x ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    // splitmix64 finaliser
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}
