//! `valuation.rs` of the legacy crate, as far as the product surface uses it: what a unit is worth to the bot (a price table and a lookup that
//! falls back to the printed cost). Pure functions of the printed records, copied.

use std::collections::BTreeMap;

use crate::observed::Observed;
use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};

/// Rough trade-good prices. Deliberately close to build cost, adjusted for utility.
///
/// A table rather than a `match`, because two units sharing a price today is a coincidence and
/// not a shared meaning: merging the arms that happen to agree would make repricing one of them
/// silently reprice the other.
pub const BASE_UNIT_VALUE: [(&str, f64); 11] = [
    ("fighter", 0.5),
    ("infantry", 1.0),
    // Hits on 6 where infantry needs 8, and sustains damage, so it is worth well more than its
    // cost of 2 suggests — which is what the cost fallback gave it.
    ("mech", 3.5),
    ("destroyer", 1.5),
    ("cruiser", 2.0),
    ("carrier", 3.5),
    ("dreadnought", 4.0),
    ("warsun", 10.0),
    ("flagship", 8.0),
    ("spacedock", 4.0),
    ("pds", 2.0),
];

/// The printed price of a base unit type, if it has one.
#[must_use]
pub fn base_unit_value(base_type: &str) -> Option<f64> {
    BASE_UNIT_VALUE
        .iter()
        .find(|(name, _)| *name == base_type)
        .map(|(_, value)| *value)
}

/// What one unit is worth, by id.
///
/// Unit upgrades and faction variants fall back to their base type, then to cost — so a newly
/// added unit is priced roughly rather than at zero, which would make it free to lose.
#[must_use]
pub fn unit_value(content: &ContentStore, sources: SourceSet, unit_id: &str) -> f64 {
    if let Some(value) = base_unit_value(unit_id) {
        return value;
    }
    // A point lookup against the store's index. This is called once per option on every
    // casualty and production choice, and building the whole catalogue to answer it was the
    // single hottest thing the bot did.
    let Some(stats) = ti4_content::units::unit_type(content, unit_id, sources) else {
        return 1.0;
    };
    base_unit_value(stats.base_type()).unwrap_or_else(|| stats.cost().max(1.0))
}

/// Bounds on any single multiplier, so no one signal can dominate a score.
pub const MULTIPLIER_FLOOR: f64 = 0.25;
/// The upper bound; see [`MULTIPLIER_FLOOR`].
pub const MULTIPLIER_CEILING: f64 = 4.0;

/// Hold a multiplier inside its bounds.
#[must_use]
pub fn clamp(value: f64) -> f64 {
    value.clamp(MULTIPLIER_FLOOR, MULTIPLIER_CEILING)
}

// -- planets -------------------------------------------------------------------------------------

/// Named, clamped context factors. Named so a score can be explained.
#[must_use]
pub fn planet_multipliers(
    seen: &Observed<'_>,
    player: &PlayerId,
    planet: &ti4_content::galaxy::Planet<'_>,
) -> BTreeMap<&'static str, f64> {
    let mut factors = BTreeMap::new();
    if !planet.tech_specialties().is_empty() {
        factors.insert("tech_specialty", 1.4);
    }
    if planet.is_legendary() {
        factors.insert("legendary", 2.0);
    }
    if planet.id() == "mr" {
        factors.insert("mecatol", 2.5); // a victory point a round via Imperial
    }
    let own_faction = seen
        .seat(player)
        .map(|seat| seat.faction.to_string())
        .unwrap_or_default();
    if planet
        .homeworld_of()
        .is_some_and(|faction| faction != own_faction)
    {
        factors.insert("enemy_homeworld", 1.3);
    }
    if has_unscored_public_goal(
        seen,
        player,
        &[
            "expand_borders",
            "subdue",
            "push_boundaries",
            "ancient_monuments",
            "lost_outposts",
        ],
    ) {
        factors.insert("public_planet_goal", 1.25);
    }
    factors.insert(
        "objective_pressure",
        clamp(1.0 + 0.15 * objective_pressure(seen, player)),
    );
    factors
}

fn has_unscored_public_goal(seen: &Observed<'_>, player: &PlayerId, aliases: &[&str]) -> bool {
    let scored = seen.scored_by(player);
    seen.revealed_objectives()
        .iter()
        .any(|goal| aliases.contains(&goal.as_str()) && !scored.contains(goal))
}

/// How close the player is to a revealed objective they could still score.
fn objective_pressure(seen: &Observed<'_>, player: &PlayerId) -> f64 {
    let scored = seen.scored_by(player);
    let outstanding = seen
        .revealed_objectives()
        .iter()
        .filter(|alias| crate::queries::objectives::has_requirement(alias))
        .any(|alias| !scored.contains(alias));
    if !outstanding {
        return 0.0;
    }
    let held = seen.controlled_planets(player).len();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a planet count is far below 2^53"
    )]
    let held = held as f64;
    (held / 3.0).min(2.0)
}

/// What taking (or holding) a planet is worth right now.
#[must_use]
pub fn planet_value(seen: &Observed<'_>, player: &PlayerId, planet: &PlanetId) -> f64 {
    let Some(record) = ti4_content::galaxy::planet(seen.content(), planet.as_str(), seen.sources())
    else {
        return 0.0;
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "printed resource and influence values are single digits"
    )]
    let base = record.resources() as f64 + 0.5 * record.influence() as f64;
    let mut total = base + 1.0; // every planet is worth something as a body count
    for factor in planet_multipliers(seen, player, &record).values() {
        total *= factor;
    }
    total
}

// -- systems -------------------------------------------------------------------------------------

/// What activating a system is worth: what can be taken, less what defends it.
#[must_use]
pub fn system_value(seen: &Observed<'_>, player: &PlayerId, system: &SystemId) -> f64 {
    let Some(galaxy) = seen.galaxy() else {
        return 0.0; // no map, so no board to read
    };
    if galaxy.coord_of(system.as_str()).is_none() {
        return 0.0; // not on this map, so there is nothing here to take
    }
    let board = seen.system(system);
    let content = seen.content();
    let sources = seen.sources();
    let types = ti4_content::units::catalogue(content, sources);

    let mut prize = 0.0;
    for record in ti4_content::galaxy::planets_in(content, system.as_str(), sources) {
        let planet = PlanetId::new(record.id());
        if board.planet_control.get(&planet) == Some(player) {
            continue; // already ours
        }
        prize += planet_value(seen, player, &planet);
    }

    let defenders: f64 = board
        .units
        .iter()
        .filter(|unit| &unit.owner != player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_ship)
        })
        .map(|unit| unit_value(content, sources, unit.type_id.as_str()))
        .sum();
    let garrison: f64 = board
        .planet_units
        .values()
        .flatten()
        .filter(|unit| &unit.owner != player)
        .map(|unit| unit_value(content, sources, unit.type_id.as_str()))
        .sum();

    prize - 0.6 * defenders - 0.4 * garrison
}

/// What this player's ships in one system are worth.
#[must_use]
pub fn fleet_strength(seen: &Observed<'_>, player: &PlayerId, system: &SystemId) -> f64 {
    let content = seen.content();
    let sources = seen.sources();
    let types = ti4_content::units::catalogue(content, sources);
    seen.system(system)
        .units_of(player)
        .into_iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_ship)
        })
        .map(|unit| unit_value(content, sources, unit.type_id.as_str()))
        .sum()
}

/// Ground forces sitting on planets with nothing carrying them anywhere.
#[must_use]
pub fn stranded_troops(seen: &Observed<'_>, player: &PlayerId) -> usize {
    let types = ti4_content::units::catalogue(seen.content(), seen.sources());
    seen.board()
        .values()
        .map(|system| {
            system
                .planet_units
                .values()
                .flatten()
                .filter(|unit| &unit.owner == player)
                .filter(|unit| {
                    types
                        .get(unit.type_id.as_str())
                        .is_some_and(ti4_content::units::UnitType::is_ground_force)
                })
                .count()
        })
        .sum()
}

