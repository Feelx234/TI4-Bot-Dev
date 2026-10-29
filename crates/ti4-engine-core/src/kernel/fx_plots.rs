//! `fx_plots`: the state half of Plots Within Plots (package A4b-2), the Firmament scoring another player's
//! secret objective as a plot (`factions::firmament::{remaining_cards, score_as_plot, place_card}`).
//!
//! A plot gives no victory point and is no scored objective: it is recorded in the seat's `plot_objectives`
//! only, after its price (`fx_pay::pay_secret`). The placement takes one of the plot cards no player has in play.

use crate::containers::SmallList;
use crate::generated::{Faction, SecretObjective};
use crate::ids::Seat;
use crate::kernel::fx_pay;
use crate::state::{CoreState, Plot, PlotCard};

/// The printed order of the plot cards (`ti4_model::plots::CARDS`: the order a placement lists them in).
pub const PRINTED: [PlotCard; 5] = [PlotCard::Enervate, PlotCard::Siphon, PlotCard::Seethe, PlotCard::Assail, PlotCard::Extract];

/// The id text of a plot card (`ti4_model::plots::CARDS`).
#[must_use]
pub const fn card_text(c: PlotCard) -> &'static str {
    match c {
        PlotCard::Enervate => "enervate",
        PlotCard::Siphon => "siphon",
        PlotCard::Seethe => "seethe",
        PlotCard::Assail => "assail",
        PlotCard::Extract => "extract",
    }
}

/// `firmament::is_firmament`: the seat plays the Firmament (not yet the Obsidian).
#[must_use]
pub fn is_firmament(core: &CoreState, seat: Seat) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.faction == Faction::TheFirmament)
}

/// `firmament::remaining_cards` (`ti4_model::plots::remaining`): the plot cards no player has in play. A card of
/// unknown identity still occupies a place, so with five cards in play nothing remains.
#[must_use]
pub fn remaining_cards(core: &CoreState) -> SmallList<PlotCard, 5> {
    let in_play: SmallList<Option<PlotCard>, 8> = core.players().iter().flat_map(|p| p.cards.plots.iter().map(|x| x.card)).collect();
    if in_play.len() >= PRINTED.len() {
        return SmallList::new();
    }
    PRINTED.iter().copied().filter(|c| !in_play.as_slice().contains(&Some(*c))).collect()
}

/// `firmament::score_as_plot`: pay the price of `secret`, which `token` scored, and record that `who` scored it as a
/// plot. `false`, nothing changed, when it is not a legal plot score.
pub fn score_as_plot(core: &mut CoreState, who: Seat, secret: SecretObjective, token: Seat) -> bool {
    let scored = core.objectives().scored;
    let mine = |s: Seat| scored[usize::from(s.0)].secret.contains(secret);
    if !is_firmament(core, who)
        || who == token
        || remaining_cards(core).is_empty()
        || !mine(token)
        || mine(who)
        || core.players().by_seat(who).is_some_and(|p| p.cards.plot_objectives.contains(secret))
    {
        return false;
    }
    // `secrets::award_plot`: the price first, then the record.
    if !fx_pay::pay_secret(core, who, secret) {
        return false;
    }
    match core.players_mut().by_seat_mut(who) {
        Some(p) => {
            p.cards.plot_objectives.insert(secret);
            true
        }
        None => false,
    }
}

/// `firmament::place_card`: put `card` with `token`'s control token on it into `owner`'s play area, facedown for the
/// Firmament and faceup for the Obsidian. `false`, nothing changed, when it is not one of the remaining cards.
pub fn place_card(core: &mut CoreState, owner: Seat, card: PlotCard, token: Seat) -> bool {
    if !remaining_cards(core).as_slice().contains(&card) {
        return false;
    }
    let faceup = !is_firmament(core, owner);
    let Some(p) = core.players_mut().by_seat_mut(owner) else { return false };
    let mut tokens = crate::containers::SeatSet::EMPTY;
    tokens.insert(token);
    p.cards.plots.push(Plot { card: Some(card), tokens, faceup });
    true
}

