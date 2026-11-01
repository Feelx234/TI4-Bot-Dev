//! The faction facts the setup reads and the three faction writes it makes (legacy
//! `factions::{keleres, obsidian, nekro, mahact, crimson, empyrean, mentak, MODULES}`).

use ti4_content::ContentStore;
use ti4_model::id::{PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;

/// The Nekro Virus (`factions::nekro::FACTION`): also the faction name in its promissory note ids.
pub const NEKRO: &str = "nekro";

/// The Empyrean (`factions::empyrean::FACTION`).
pub const EMPYREAN: &str = "empyrean";

/// The three Keleres variant aliases, in Tribuni order (`factions::keleres::VARIANTS`).
pub const KELERES_VARIANTS: [&str; 3] = ["keleresm", "keleresx", "keleresa"];

/// Factions whose setup choice is refused by their own text (Nocturne;
/// `factions::obsidian::NOT_CHOOSABLE_DURING_SETUP`).
pub const NOT_CHOOSABLE_DURING_SETUP: &[&str] = &["obsidian"];

/// The alias of every legacy faction module in `factions::MODULES` order: the roster the wide
/// seating draws from. A parity test compares it with the legacy table.
pub const MODULE_ALIASES: [&str; 27] = [
    "arborec", "argent", "bastion", "cabal", "crimson", "deepwrought", "empyrean", "firmament",
    "ghost", "keleresm", "keleresx", "keleresa", "mahact", "mentak", "muaat", "naalu", "naaz",
    "nekro", "nomad", "obsidian", "ralnel", "saar", "sardakk", "titans", "winnu", "yin", "yssaril",
];

/// Whether `faction` is any Keleres variant.
#[must_use]
pub fn is_keleres_faction(faction: &str) -> bool {
    KELERES_VARIANTS.contains(&faction)
}

/// Whether a content record's `faction` tag belongs to `alias`: the alias itself, or, for a Keleres
/// variant, the family `keleres` (and, for units, the first variant `keleresm`).
#[must_use]
pub fn tag_belongs_to(tag: &str, alias: &str, unit: bool) -> bool {
    tag.eq_ignore_ascii_case(alias)
        || (is_keleres_faction(alias)
            && (tag.eq_ignore_ascii_case("keleres")
                || (unit && tag.eq_ignore_ascii_case("keleresm"))))
}

/// Mahact, Hubris: "During setup, purge your Alliance promissory note."
#[must_use]
pub fn purges_alliance(faction: &str) -> bool {
    faction == "mahact"
}

/// Sorrow: "place a inactive breach there" (the Sorrow, tile 94). Idempotent.
pub fn place_sorrow_breach(state: &mut GameState) {
    let sorrow = SystemId::new(crate::seating::SORROW);
    state.breach_tokens.insert(sorrow.clone());
    state
        .faction_marks
        .remove(&format!("crimson:breach:active:{sorrow}"));
}

/// Empyrean Dark Whispers: "During setup, take the additional Empyrean faction promissory note."
/// A no-op for every other faction.
pub fn deal_empyrean_notes(state: &mut GameState, content: &ContentStore, player: &PlayerId) {
    if !state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == EMPYREAN)
    {
        return;
    }
    let aliases: Vec<String> = ti4_content::factions::get(content, EMPYREAN)
        .map(|faction| {
            faction
                .promissory_notes()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for alias in aliases {
        state
            .promissory_notes
            .entry(super::promissory::note_id(&alias, EMPYREAN))
            .or_insert_with(|| player.clone());
    }
}

/// The Table's Grace (Mentak, Thunder's Edge): with Cruiser II the Corsair stands in for it
/// (`factions::mentak::unit_form_override`). `None` for every other seat and base type.
#[must_use]
pub fn unit_form_override(
    state: &GameState,
    player: &PlayerId,
    base_type: &str,
    chosen_id: &str,
) -> Option<UnitTypeId> {
    (base_type == "cruiser"
        && chosen_id == "cruiser2"
        && state
            .player(player)
            .is_some_and(|seat| seat.faction.as_str() == "mentak")
        && state
            .player(player)
            .and_then(|seat| seat.breakthrough.as_ref())
            .is_some_and(|held| held.as_str() == "mentakbt"))
    .then(|| UnitTypeId::new("mentak_cruiser3"))
}
