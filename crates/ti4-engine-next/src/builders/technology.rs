//! Gaining a technology outright while a position is built (legacy `technology::{grant, gain,
//! apply_unit_upgrades, unit_swaps}`, the string bodies).

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, TechnologyId, UnitTypeId};
use ti4_model::state::GameState;

/// Gain a technology outright (90.5), without checking prerequisites.
pub fn grant(state: &mut GameState, player: &PlayerId, alias: &TechnologyId) {
    if let Some(seat) = state.player_mut(player) {
        seat.technologies.insert(alias.clone());
    }
}

/// Gain a technology by any route other than research, and let a unit upgrade reach the units
/// already on the board (90.8).
pub fn gain(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    alias: &TechnologyId,
) {
    grant(state, player, alias);
    apply_unit_upgrades(state, content, sources, player);
}

/// The unit type swaps a player's upgrades call for: each unit type id in scope that must be
/// replaced, and the id it becomes (the upgrade owned, else the faction's own version of a plain
/// unit, then any faction override).
#[must_use]
pub fn unit_swaps(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> BTreeMap<String, String> {
    let mut swaps: BTreeMap<String, String> = BTreeMap::new();
    let Some(seat) = state.player(player) else {
        return swaps;
    };
    let faction = seat.faction.to_string();
    let held: Vec<String> = seat
        .technologies
        .iter()
        .map(|tech| tech.as_str().to_owned())
        .collect();

    let catalogue = ti4_content::units::catalogue(content, sources);
    for kind in catalogue.values() {
        let base = kind.base_type();
        // No upgrade owned: a generic base unit becomes the faction's own version where the
        // faction has one (Sol's `sol_carrier` for `carrier`), so a seat never holds both.
        let own_base = || {
            if kind.faction().is_none() && kind.required_technology().is_none() {
                ti4_content::units::faction_unit(content, &faction, base, sources)
                    .map(|own| own.id().to_owned())
            } else {
                None
            }
        };
        let chosen = ti4_content::units::unlocked_upgrade(content, sources, base, &faction, &held)
            .map(|better| better.id().to_owned())
            .or_else(own_base)
            .unwrap_or_else(|| kind.id().to_owned());
        // A module may name another form for this base type (Mentak Corsair's acquisition).
        let chosen = super::factions::unit_form_override(state, player, base, &chosen)
            .map_or(chosen, |form| form.as_str().to_owned());
        if chosen != kind.id() {
            swaps.insert(kind.id().to_owned(), chosen);
        }
    }
    swaps
}

/// Replace this player's units on the board with the versions their upgrades unlock (90.8).
pub fn apply_unit_upgrades(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) {
    let swaps = unit_swaps(state, content, sources, player);
    if swaps.is_empty() {
        return;
    }
    for board in state.board.values_mut() {
        for unit in &mut board.units {
            if unit.owner == *player
                && let Some(better) = swaps.get(unit.type_id.as_str())
            {
                unit.type_id = UnitTypeId::new(better);
            }
        }
        for standing in board.planet_units.values_mut() {
            for unit in standing {
                if unit.owner == *player
                    && let Some(better) = swaps.get(unit.type_id.as_str())
                {
                    unit.type_id = UnitTypeId::new(better);
                }
            }
        }
    }
}
