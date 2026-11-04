//! From `invasion.rs` of the legacy crate (copies).

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::invasion::{GROUND_CASUALTY_KIND};

use ti4_content::ContentStore;


/// Display only: a breakthrough as an offer-card header: its name and the first sentence of its
/// printed text (the ability that is being offered).
pub fn breakthrough_card(content: &ContentStore, card: &str) -> serde_json::Value {
    let record = content.get(ti4_model::content_types::ContentType::Breakthroughs, card);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key));
    let first_sentence = field("text").map(|text| match text.find(". ") {
        Some(end) => &text[..=end],
        None => text,
    });
    crate::wire::choice::offer_card(
        field("name").unwrap_or(card),
        "breakthrough",
        None,
        first_sentence,
    )
}

/// The invasion texts of the ability table (the thin copy of `invasion_live::fill_codes` of the legacy crate). The legacy lockstep tests mint the
/// texts of the position (`ground`, the combat values, the unit names of the dice groups) in the state's text table; a product session has one
/// table, the render's, which `Render::sync_codes` continues into the core's, so every text is the render's. Without them every ground-dice
/// question was labelled by code 0 (`ground_combat` dice, Fire Team's question).
pub fn fill_codes(table: &mut ti4_engine_core::kernel::ability::AbilityTable, r: &crate::render::Render) {
    use crate::generated::IdStr;
    let c = &mut table.codes.invasion;
    c.ground = r.code("ground");
    c.bombardment = r.code("bombardment");
    c.ground_round = r.code("ground_round");
    c.space_cannon_defense = r.code("space_cannon_defense");
    c.bombardment_cause = r.code("bombardment");
    c.space_cannon_cause = r.code("space_cannon_defense");
    c.planets = r.code("planets");
    c.dunlain_reaper = r.code("dunlain_reaper");
    c.minister = r.code("minister_exploration");
    for (slot, text) in c.phases.iter_mut().zip(["bombardment", "custodians", "landing", "ground_battle", "planet_result", "next_defender", "control", "complete"]) {
        *slot = r.code(text);
    }
    for (i, slot) in c.names.iter_mut().enumerate() {
        *slot = r.code(&format!("combat value {}", i + 1));
    }
    for unit in ti4_engine_core::generated::Unit::ALL {
        c.units[unit as usize] = r.code(IdStr::as_str(unit));
        c.shown_units[unit as usize] = r.code(IdStr::as_str(unit));
    }
}
