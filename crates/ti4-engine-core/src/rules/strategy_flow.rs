//! The strategy phase flow, the mutation half (package B8; string `draft`, `phase`, `state`, `strategy`): a
//! card taken from the mat (with the trade goods on it), cards dealt, exhausted, exchanged and dropped, the
//! mat stocked with a trade good per unpicked card, the mat laid out again for the next round.
//!
//! Each function is the body of one string function over the slices `Meta`-less `Players` and `Strategy`
//!.

use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::state::CoreState;

/// The initiative sort of `GameState::initiative_sorted`: printed number, 99 when the card has none. Stable.
fn sorted_by_initiative(core: &CoreState, cards: &mut [StrategyCard]) {
    let init = &core.strategy().initiative;
    cards.sort_by_key(|c| init.get(c).copied().unwrap_or(99));
}

/// `GameState::deal_strategy_card`: the card joins the holding in initiative order. `false`: no such seat.
pub fn deal(core: &mut CoreState, who: Seat, card: StrategyCard) -> bool {
    let Some(p) = core.players().by_seat(who) else { return false };
    let mut held: Vec<StrategyCard> = p.strat.as_slice().to_vec();
    held.push(card);
    sorted_by_initiative(core, &mut held);
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.strat = held.into_iter().collect();
    }
    true
}

/// `GameState::exhaust_strategy_card`: `true` when the card was not exhausted yet.
pub fn exhaust(core: &mut CoreState, who: Seat, card: StrategyCard) -> bool {
    core.players_mut().by_seat_mut(who).is_some_and(|p| p.strat_exhausted.insert(card))
}

/// `GameState::swap_strategy_card`: `out` leaves the holding, `into` joins it in initiative order, and an
/// exhausted `out` hands its exhaustion to `into`.
pub fn swap(core: &mut CoreState, who: Seat, out: StrategyCard, into: StrategyCard) -> bool {
    let Some(p) = core.players().by_seat(who) else { return false };
    let mut held: Vec<StrategyCard> = p.strat.as_slice().iter().copied().filter(|c| *c != out).collect();
    held.push(into);
    sorted_by_initiative(core, &mut held);
    let mut spent = p.strat_exhausted;
    if spent.remove(out) {
        spent.insert(into);
    }
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.strat = held.into_iter().collect();
        p.strat_exhausted = spent;
    }
    true
}

/// `GameState::clear_strategy_cards`.
pub fn clear(core: &mut CoreState, who: Seat) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.strat.clear();
        p.strat_exhausted.clear();
    }
}

/// `strategy::exchange_strategy_cards`: two seats trade one card each; each card keeps its own exhaustion.
/// `false` (nothing changed) for the same seat or card twice, a seat not there, or a card not held.
pub fn exchange(core: &mut CoreState, a: Seat, card_a: StrategyCard, b: Seat, card_b: StrategyCard) -> bool {
    if a == b || card_a == card_b {
        return false;
    }
    let (Some(pa), Some(pb)) = (core.players().by_seat(a), core.players().by_seat(b)) else { return false };
    if !pa.strat.as_slice().contains(&card_a) || !pb.strat.as_slice().contains(&card_b) {
        return false;
    }
    let (spent_a, spent_b) = (pa.strat_exhausted.contains(card_a), pb.strat_exhausted.contains(card_b));
    swap(core, a, card_a, card_b);
    swap(core, b, card_b, card_a);
    for (who, gave, got, got_spent) in [(a, card_a, card_b, spent_b), (b, card_b, card_a, spent_a)] {
        if let Some(p) = core.players_mut().by_seat_mut(who) {
            p.strat_exhausted.remove(gave);
            p.strat_exhausted.remove(got);
            if got_spent {
                p.strat_exhausted.insert(got);
            }
        }
    }
    true
}

/// `draft::take_strategy_card` after its checks: the trade goods on the card go to the picker, the card is
/// dealt and leaves the mat. The `TRADE_GOODS_GAINED` event the gain announces is staged by the string caller
/// (staged events are not a slice the codec holds). `None`: the card is not on the mat, or the seat is not
/// there (nothing changed).
pub fn take_card(core: &mut CoreState, who: Seat, card: StrategyCard) -> Option<()> {
    core.players().by_seat(who)?;
    let index = core.strategy().unclaimed.as_slice().iter().position(|c| *c == card)?;
    let goods = core.strategy_mut().goods.remove(&card).unwrap_or(0);
    if goods > 0 {
        // The string adds to the counter unclamped; the core counter is an i16 (`gain_trade_goods`).
        fx::gain_trade_goods(core, who, goods);
    }
    deal(core, who, card);
    core.strategy_mut().unclaimed.remove(index);
    Some(())
}

/// `phase::stock_unclaimed_cards` (83.4): a trade good on every card still on the mat.
pub fn stock_unclaimed(core: &mut CoreState) {
    let cards: Vec<StrategyCard> = core.strategy().unclaimed.as_slice().to_vec();
    for c in cards {
        let n = core.strategy().goods.get(&c).copied().unwrap_or(0);
        core.strategy_mut().goods.insert(c, n + 1);
    }
}

/// `strategy_cards::replenish` without the announcements: the commodities go to the faction's printed value
/// (`commodity_limit`; a seat above it is set down to it, as the string does). Returns how many were gained.
/// `0` for a seat that is not there. The Siphon event, Cabal's replenish note and the Trade Agreement hook are
/// the string callers' (the adapter bails where one applies).
pub fn replenish(core: &mut CoreState, who: Seat) -> i32 {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return 0 };
    let limit = i16::from(p.faction.data().commodities);
    let gained = (limit - p.econ.commodities).max(0);
    p.econ.commodities = limit;
    i32::from(gained)
}

/// The mat of `phase::begin_next_round`: `cards` in the order given, without the ones a seat under Political
/// Stability keeps; the initiative overrides lapse.
pub fn lay_out_mat(core: &mut CoreState, cards: &[StrategyCard]) {
    let held: Vec<StrategyCard> = core
        .players()
        .iter()
        .filter(|p| p.econ.stability)
        .flat_map(|p| p.strat.as_slice().iter().copied())
        .collect();
    let st = core.strategy_mut();
    st.initiative_overrides.clear();
    st.unclaimed = cards.iter().copied().filter(|c| !held.contains(c)).collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    fn core() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        for (i, c) in StrategyCard::ALL[..8].iter().enumerate() {
            s.strategy.initiative.insert(*c, i as i32 + 1);
        }
        s.strategy.unclaimed = StrategyCard::ALL[..8].iter().copied().collect();
        s
    }

    #[test]
    fn take_collects_goods_and_deals_in_initiative_order() {
        let mut c = core();
        let (a, b) = (StrategyCard::ALL[0], StrategyCard::ALL[5]);
        c.strategy.goods.insert(b, 3);
        take_card(&mut c, Seat(0), b).unwrap();
        take_card(&mut c, Seat(0), a).unwrap();
        let p = c.players().by_seat(Seat(0)).unwrap();
        assert_eq!(p.strat.as_slice(), [a, b]);
        assert_eq!(p.econ.trade_goods, 3);
        assert_eq!(c.strategy().unclaimed.len(), 6);
        assert!(c.strategy().goods.is_empty());
        assert!(take_card(&mut c, Seat(0), a).is_none());
    }

    #[test]
    fn stock_adds_one_per_unclaimed_card() {
        let mut c = core();
        c.strategy.unclaimed.truncate(2);
        stock_unclaimed(&mut c);
        stock_unclaimed(&mut c);
        assert_eq!(c.strategy().goods.get(&StrategyCard::ALL[0]), Some(&2));
        assert_eq!(c.strategy().goods.len(), 2);
    }

    #[test]
    fn exchange_keeps_each_cards_exhaustion() {
        let mut c = core();
        let (a, b) = (StrategyCard::ALL[1], StrategyCard::ALL[2]);
        deal(&mut c, Seat(0), a);
        deal(&mut c, Seat(1), b);
        exhaust(&mut c, Seat(0), a);
        assert!(exchange(&mut c, Seat(0), a, Seat(1), b));
        assert!(c.players().by_seat(Seat(1)).unwrap().strat_exhausted.contains(a));
        assert!(!c.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(b));
        assert!(!exchange(&mut c, Seat(0), a, Seat(1), b));
    }
}
