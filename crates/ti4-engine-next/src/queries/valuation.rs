//! `valuation.rs` of the legacy crate, as far as the product surface uses it: what a unit is worth to the bot (a price table and a lookup that
//! falls back to the printed cost). Pure functions of the printed records, copied.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;

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
