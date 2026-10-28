//! firmament: tier W: offers, asks and `*_performed` (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only (the window owner writes only this file, after a request to the package owner).
//!
//! Ported here: `firmament_place_card` (state change, no ask).
//!
//! Left for the window owners, with the reason:
//!
//! * `firmament_plot_options` (`plot_options`) and `firmament_score_as_plot` (`score_as_plot`): need `secrets::fulfils`
//!   and `secrets::award_plot` (B5, the objectives area).
//! * `timing_abilities` (Myru Vos, Heaven's Eye repairs, the Sowing, the Parasite, the Planesplitter, Black Ops),
//!   `commander_unlocked`, `leader_action`, `use_leader`, `mapped_component_actions` / `perform_component`
//!   (`firmament_flip`): the asks of B7 and the reaction windows of B1.
//! * `firmament_plots` abilities (Seethe, Extract, Siphon, Enervate follow, `primary_instead_offered`): the Obsidian's
//!   (package CE), asks of B4/B7.

use ti4_engine_core::containers::SeatSet;
use ti4_engine_core::generated::Faction;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::state::{Plot, PlotCard};

use super::query::remaining;
use crate::common::is_faction;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.firmament_place_card.push(place_card);
}

/// The card with this index (the text order of the ids), if there is one.
fn plot_card(index: u16) -> Option<PlotCard> {
    Some(match index {
        0 => PlotCard::Assail,
        1 => PlotCard::Enervate,
        2 => PlotCard::Extract,
        3 => PlotCard::Seethe,
        4 => PlotCard::Siphon,
        _ => return None,
    })
}

/// Put plot card `card` with `token`'s control token on it into `owner`'s play area: facedown for the Firmament,
/// faceup for the Obsidian. `false`, changing nothing, when that card is not one of the remaining cards.
fn place_card(cx: &mut Cx<'_>, owner: Seat, card: u16, token: Seat) -> bool {
    let Some(card) = plot_card(card) else { return false };
    if !remaining(cx.state)[card as usize] {
        return false;
    }
    let faceup = !is_faction(cx.state, owner, Faction::TheFirmament);
    let mut tokens = SeatSet::EMPTY;
    tokens.insert(token);
    let Some(p) = cx.state.players_mut().by_seat_mut(owner) else { return false };
    p.cards.plots.push(Plot { card: Some(card), tokens, faceup });
    true
}
