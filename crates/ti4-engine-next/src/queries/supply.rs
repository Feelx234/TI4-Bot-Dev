//! `supply.rs` of the legacy crate, as far as the product surface uses it: the plastic a player has and how much of it is still in the box.
//! Plain reads of the public state and the printed records, copied.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, UnitTypeId};
use ti4_model::state::GameState;

/// Anything not listed here is uncapped, which is how fighters and infantry pass through.
const UNCAPPED: i64 = 99;

/// Plastic per player, fourth edition plus Prophecy of Kings, keyed by base type.
#[must_use]
pub fn plastic(base_type: &str) -> Option<i64> {
    let count = match base_type {
        "flagship" => 1,
        "warsun" => 2,
        "dreadnought" => 5,
        "cruiser" | "destroyer" => 8,
        "carrier" | "mech" => 4,
        "pds" => 6,
        "spacedock" => 3,
        _ => return None,
    };
    Some(count)
}

/// The plastic a unit id corresponds to, read from the corpus rather than by matching on the id.
#[must_use]
pub fn base_type_of(content: &ContentStore, sources: SourceSet, unit: &UnitTypeId) -> String {
    ti4_content::units::catalogue(content, sources).get(unit.as_str()).map_or_else(|| unit.as_str().to_owned(), |kind| kind.base_type().to_owned())
}

/// How many pieces of this base type the player has out of the box: the space areas and the planets, the captured units of others held by
/// other players, and the ships on the Crimson hero card.
#[must_use]
pub fn held(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, base_type: &str) -> i64 {
    let types = ti4_content::units::catalogue(content, sources);
    let matches_base = |unit: &UnitTypeId| types.get(unit.as_str()).map_or(unit.as_str(), |kind| kind.base_type()) == base_type;
    let mut total = 0;
    let mut count = |unit: &ti4_model::units::Unit| {
        if &unit.owner == player && matches_base(&unit.type_id) {
            total += 1;
        }
    };
    for board in state.board.values() {
        for unit in &board.units {
            count(unit);
        }
        for units in board.planet_units.values() {
            for unit in units {
                count(unit);
            }
        }
    }
    for captor in &state.players {
        for (owner, unit) in &captor.captured_units {
            if owner == player && matches_base(unit) {
                total += 1;
            }
        }
    }
    // Ships on the Crimson hero card are off the board and still out of the box.
    total + held_on_card(state, content, sources, player, base_type)
}

/// How many pieces of `base_type` the player has on the Crimson hero card (`faction_marks`, a comma list of unit ids).
fn held_on_card(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, base_type: &str) -> i64 {
    let Some(ships) = state.faction_marks.get(&format!("crimson:card:{player}")) else { return 0 };
    let types = ti4_content::units::catalogue(content, sources);
    i64::try_from(ships.split(',').filter(|ship| !ship.is_empty()).filter(|ship| types.get(*ship).map_or(*ship, |kind| kind.base_type()) == base_type).count()).unwrap_or(0)
}

/// How many more of this unit may be placed before the box is empty.
#[must_use]
pub fn remaining(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, unit: &UnitTypeId) -> i64 {
    let base = base_type_of(content, sources, unit);
    let Some(limit) = plastic(&base) else {
        return UNCAPPED;
    };
    (limit - held(state, content, sources, player, &base)).max(0)
}
