//! Fleet composition reads of the surface (`opening::fleet_of`, content and the board only).

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

/// `(ships that can carry something, ground forces)` of one player, wherever they stand. Capacity comes from the content, not from a
/// list of hull names; infantry being transported sit in the space area and still count.
#[must_use]
pub fn opening_fleet(state: &GameState, player: &PlayerId, content: &ContentStore, sources: SourceSet) -> (usize, usize) {
    let mut capacity_ships = 0;
    let mut infantry = 0;
    let ground = |unit: &ti4_model::units::Unit| -> bool {
        let kind = unit.type_id.as_str();
        kind.ends_with("infantry") || kind.ends_with("mech")
    };
    for system in state.board.values() {
        for unit in system.units.iter().filter(|unit| &unit.owner == player) {
            if ground(unit) {
                infantry += 1;
            } else if super::transit::capacity_of(content, sources, unit) > 0 {
                capacity_ships += 1;
            }
        }
        infantry += system.planet_units.values().flatten().filter(|unit| &unit.owner == player && ground(unit)).count();
    }
    (capacity_ships, infantry)
}
