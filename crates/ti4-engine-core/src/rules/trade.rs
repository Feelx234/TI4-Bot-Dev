//! Transactions that change the typed state (`transactions.rs`: `take`, `give`, the per-turn and
//! per-round records). Package B4.
//!
//! Only what lives in the typed slices moves here: trade goods, commodities, relic fragments, an
//! action card, a secret objective. A promissory note and a planet card change hands in slices
//! other packages own (cards, board); the adapter keeps those on the string engine.

use crate::generated::ExploreType;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::Kernel;
use crate::query::trade::{Env, Offer, Terms};
use crate::state::{CoreState, TransientFlags};

/// The hooks of the A5 table answer the two slots that exist there; the rest is neutral.
pub struct KernelEnv<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
}

impl Env for KernelEnv<'_> {
    fn has_iihq(&self, _: Seat) -> bool {
        false
    }
    fn wide(&self, who: Seat) -> bool {
        self.kernel.hooks.general.ignores_neighbours(self.core, self.kernel, who)
    }
    fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn trades_action_cards(&self, who: Seat) -> bool {
        self.kernel.hooks.general.trades_action_cards(self.core, self.kernel, who)
    }
    fn black_market(&self) -> bool {
        self.core.transient().misc.flags.has(TransientFlags::BLACK_MARKET)
    }
}



/// `transactions::take`: remove what a seat gives away (goods, commodities, fragments, the first
/// matching action card and secret).
pub fn take(core: &mut CoreState, seat: Seat, terms: &Terms) {
    let Some(p) = core.players_mut().by_seat_mut(seat) else { return };
    if let Some(card) = terms.action_card
        && let Some(at) = p.cards.hand.as_slice().iter().position(|c| *c == card)
    {
        p.cards.hand.remove(at);
    }
    if let Some(secret) = terms.secret
        && let Some(at) = p.cards.secrets.as_slice().iter().position(|c| *c == secret)
    {
        p.cards.secrets.remove(at);
    }
    p.econ.trade_goods -= i16::try_from(terms.trade_goods).unwrap_or(0);
    p.econ.commodities -= i16::try_from(terms.commodities).unwrap_or(0);
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        let n = terms.fragments[kind as usize];
        if n > 0
            && let Some(held) = p.cards.fragments.get(kind)
        {
            p.cards.fragments.set(kind, held - i16::try_from(n).unwrap_or(0));
        }
    }
    // `relic_fragments.retain(|_, held| *held > 0)`: a spent kind is gone, not zero.
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        if p.cards.fragments.get(kind).is_some_and(|n| n <= 0) {
            p.cards.fragments.remove(kind);
        }
    }
}

/// `transactions::give`: hand over what a seat receives. A commodity becomes a trade good on
/// arrival (LRR 21.5).
pub fn give(core: &mut CoreState, seat: Seat, terms: &Terms) {
    {
        let Some(p) = core.players_mut().by_seat_mut(seat) else { return };
        if let Some(card) = terms.action_card {
            p.cards.hand.push(card);
        }
        if let Some(secret) = terms.secret {
            p.cards.secrets.push(secret);
        }
        for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
            let n = terms.fragments[kind as usize];
            if n > 0 {
                let held = p.cards.fragments.get(kind).unwrap_or(0);
                p.cards.fragments.set(kind, held + i16::try_from(n).unwrap_or(0));
            }
        }
    }
    fx::gain_trade_goods(core, seat, terms.trade_goods + terms.commodities);
}

/// The goods half of `transactions::resolve`: both sides are taken before either is given.
pub fn exchange(core: &mut CoreState, offer: &Offer) {
    take(core, offer.proposer, &offer.given);
    take(core, offer.partner, &offer.received);
    give(core, offer.partner, &offer.given);
    give(core, offer.proposer, &offer.received);
}

/// `GameState::record_transaction`: LRR 94.1 limits each pair, so both sides remember it.
pub fn record_transaction(core: &mut CoreState, a: Seat, b: Seat) {
    let t = core.trade_mut();
    t.transactions_this_turn.get_or_insert_with(a, Default::default).insert(b);
    t.transactions_this_turn.get_or_insert_with(b, Default::default).insert(a);
}

/// `GameState::note_negotiation`: one negotiation `seat` initiated, this action and this round.
pub fn note_negotiation(core: &mut CoreState, seat: Seat) {
    let t = core.trade_mut();
    for tally in [&mut t.negotiations_this_action, &mut t.negotiations_this_round] {
        let n = tally.get_or_insert_with(seat, || 0);
        *n = n.saturating_add(1);
    }
}

/// `GameState.transactions_this_round.push((proposer, partner))` (duplicates kept).
pub fn record_resolved(core: &mut CoreState, proposer: Seat, partner: Seat) {
    core.trade_mut().this_round.edit().push((proposer, partner));
}
