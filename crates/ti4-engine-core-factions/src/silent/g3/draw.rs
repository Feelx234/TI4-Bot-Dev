//! The plain draw of action cards an ability of this lane makes (`action_cards::draw`): up to `count` cards into the hand. The string's draw also asks for the bonus card and the
//! discard of Yssaril's Scheming and for the discard over the hand limit; a rule of the table cannot ask those from inside an effect, so where one of them would come the draw is a LOUD
//! gap (`Unknown::AbilityGap` of the rule's own slot) and nothing is drawn.

use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{Eff, Slot, Unknown};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::query::reach;

/// Draw `count` cards for `who` unless the draw would ask something (a bonus card, Scheming's discard, the hand over its limit). `false`: it would, `slot` is reported, nothing was drawn.
pub fn plain(eff: &mut Eff<'_, '_>, who: Seat, count: usize, slot: Slot) -> bool {
    let (s, k) = eff.read();
    let bonus = k.hooks.economy.action_card_draw_bonus(s, k, who, count);
    let held = s.players().by_seat(who).map_or(0, |p| p.cards.hand.len());
    let limit = ti4_engine_core::rules::action_cards::hand_limit(s, reach::blanked(s), |l| k.hooks.economy.action_card_limit(s, k, who, l));
    let scheming = s.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&ti4_engine_core::generated::Ability::Scheming));
    let asks = bonus > 0 || scheming || held + count > limit;
    if asks {
        eff.unknown(Unknown::AbilityGap(slot));
        return false;
    }
    // (From rules version 30 the unofficial cards leave the deck before a draw; only the latest version is supported.)
    fx::draw_action_cards(eff.state(), who, count, true);
    true
}
