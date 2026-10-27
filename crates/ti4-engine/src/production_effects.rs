//! What factions and cards change about paying for and placing production, in one place.
//!
//! The production and payment windows used to call the factions that touch them (Keleres'
//! agent commodities, the Deepwrought's ocean cards, the trade-good worth hook, remote
//! destinations, the Last Bastion's Icon, the Crimson card spot) directly. Each function here is
//! the expression that was at the call site, moved without change: one seam per effect.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_content::units::UnitType;

use crate::production::{REMOTE_SEPARATOR, SPACE};

/// Commodities `player` may spend as trade goods right now (Keleres' agent).
pub(crate) fn spendable_commodities(state: &GameState, player: &PlayerId) -> i64 {
    crate::factions::keleres::spendable_commodities(state, player)
}

/// Planet cards off the map that `player` may still exhaust as payment (the Deepwrought's oceans).
pub(crate) fn spendable_off_map_cards(state: &GameState, player: &PlayerId) -> Vec<PlanetId> {
    crate::factions::deepwrought::spendable_oceans(state, player)
}

/// What one trade good is worth as payment: two with the `mc` technology, else one.
pub(crate) fn trade_good_worth(state: &GameState, player: &PlayerId) -> i64 {
    // Mentak `mc` lives in `factions/mentak.rs` on the hook below; nothing is hard-coded here.
    let base = 1;
    crate::factions::hooks_economy::trade_good_worth(state, player, base)
}

/// Add to `spots` the places other than the producing system where a `kind` unit may go: module
/// destinations, then, for ships, the Last Bastion's Icon and the Crimson card spot. The caller
/// has already checked that the system can produce the unit at all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_remote_spots(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    kind: UnitType<'_>,
    produced: &[(UnitTypeId, String)],
    spots: &mut Vec<String>,
) {
    spots.extend(
        crate::factions::hooks_economy::production_destinations(
            state,
            content,
            sources,
            player,
            system,
            kind.base_type(),
        )
        .into_iter()
        .map(|(system, planet)| {
            let at = planet.map_or_else(|| SPACE.to_owned(), |planet| planet.to_string());
            format!("{system}{REMOTE_SEPARATOR}{at}")
        }),
    );
    if kind.is_ship() {
        // The Last Bastion's Icon: "place those ships in a system that contains 1 of your
        // command tokens, ...". Shapes the ship spots over the whole use.
        crate::factions::bastion::adjust_ship_spots(
            state, content, sources, player, system, produced, spots,
        );
        // The Crimson hero: "You may place any of those ships onto this card."
        crate::factions::crimson_cards::offer_card_spot(state, player, spots);
    }
}
