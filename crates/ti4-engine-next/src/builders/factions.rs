//! The faction facts the setup reads and the three faction writes it makes (legacy
//! `factions::{keleres, obsidian, nekro, mahact, crimson, empyrean, mentak, MODULES}`).

use ti4_content::ContentStore;
use ti4_model::id::{PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;

pub use ti4_engine_core_setup::factions::{EMPYREAN, KELERES_VARIANTS, MODULE_ALIASES, NEKRO, NOT_CHOOSABLE_DURING_SETUP, is_keleres_faction, purges_alliance, tag_belongs_to};

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
