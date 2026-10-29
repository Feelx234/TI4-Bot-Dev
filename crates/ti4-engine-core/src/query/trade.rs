//! Transactions between neighbours on the typed state (`transactions.rs`, LRR 60 and 94): who is a
//! neighbour, who may deal with whom, what a deal may be paid with and why a deal is not legal.
//!
//! Package B4. Every function names the string function it answers and returns the same value in
//! the same order; the dual run (`Area::TradeDiplomacy`) compares them. Three kinds of facts reach
//! the rules from outside, through [`Env`], because their owners are not ported yet: the answers
//! of the faction hooks (I.I.H.Q. Modernization, Guild Ships, Trade Convoys, Arbiters, the
//! per-turn limit exemption), the promissory-note and planet-card legs of a deal
//! ([`Terms::note`], [`Terms::planet`], computed by the string engine and handed over as facts),
//! and the map's adjacency ([`Adjacency`], cached per galaxy by the adapter).
//! `rules::trade::KernelEnv` answers the hooks from the A5 hook table; it is what the rules run on
//! once the faction modules register.

use crate::bits::SysSet;
use crate::containers::SmallList;
use crate::galaxy::Adjacency;
use crate::generated::{ActionCard, ExploreType, SecretObjective, System};
use crate::ids::Seat;
use crate::state::{CoreState, Phase};

/// Seats in an order.
pub type SeatList = SmallList<Seat, 8>;

/// Per-turn and per-round limits of negotiations (`ti4_model::state::MAX_NEGOTIATIONS_PER_*`; the
/// adapter passes the model's values, the core does not redefine them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub per_action: u8,
    pub per_round: u8,
}

/// What the rules cannot see in the typed state: the answers of the faction hooks.
pub trait Env {
    /// The seat holds I.I.H.Q. Modernization (`keleres::has_iihq`).
    fn has_iihq(&self, who: Seat) -> bool;
    /// The seat may deal with anybody: Guild Ships or the Trade Convoys note
    /// (`faction_abilities::ignores_neighbours || promissory::reaches_anyone`).
    fn wide(&self, who: Seat) -> bool;
    /// A module lets this pair deal whatever the map says (`hooks_cards::transaction_reach`).
    fn transaction_reach(&self, a: Seat, b: Seat) -> bool;
    /// A module exempts the pair from the per-turn limit (`hooks_cards::transaction_exempt_from_limit`).
    fn exempt_from_limit(&self, a: Seat, b: Seat) -> bool;
    /// The seat may trade action cards (Arbiters; `transactions::trades_action_cards`).
    fn trades_action_cards(&self, who: Seat) -> bool;
    /// Black Market Dealings marks this negotiation (`TransientFlags::BLACK_MARKET`).
    fn black_market(&self) -> bool;
}

/// Everything a trade query reads.
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    pub adj: &'a Adjacency,
    /// The Mecatol Rex system of the galaxy (`seating::mecatol_in_galaxy`).
    pub mecatol: System,
    pub limits: Limits,
    pub env: &'a dyn Env,
}

// -- presence and neighbours ------------------------------------------------------------------

/// `transactions::presence`: systems where the seat has a unit (in space or on a planet) or
/// controls a planet.
#[must_use]
pub fn presence(core: &CoreState, seat: Seat) -> SysSet {
    let mut out = super::board::systems_with_units_of(core, seat);
    for (planet, cell) in core.board().planets() {
        if cell.control == Some(seat) || cell.units.as_slice().iter().any(|u| u.owner() == seat) {
            if let Some(system) = cell.placed_in.or(planet.data().system) {
                out.insert(system.index());
            }
        }
    }
    out
}

/// `transactions::present_together`: two presences share a system or occupy adjacent ones.
#[must_use]
pub fn present_together(adj: &Adjacency, here: &SysSet, there: &SysSet) -> bool {
    here.intersects(there) || here.iter().any(|s| adj.of(crate::ids::system_at(s)).intersects(there))
}

/// `keleres::near_mecatol`: units or planets in or next to the Mecatol Rex system.
#[must_use]
pub fn near_mecatol(cx: &Cx<'_>, seat: Seat) -> bool {
    near_mecatol_of(cx, &presence(cx.core, seat))
}

fn near_mecatol_of(cx: &Cx<'_>, presence: &SysSet) -> bool {
    presence.contains(cx.mecatol.index()) || cx.adj.of(cx.mecatol).intersects(presence)
}

/// `keleres::mecatol_neighbours`: I.I.H.Q. Modernization makes the holder a neighbour of everyone
/// in or next to Mecatol Rex; mutual.
#[must_use]
pub fn mecatol_neighbours(cx: &Cx<'_>, a: Seat, b: Seat) -> bool {
    (cx.env.has_iihq(a) && near_mecatol(cx, b)) || (cx.env.has_iihq(b) && near_mecatol(cx, a))
}

/// `transactions::are_neighbours` (LRR 60).
#[must_use]
pub fn are_neighbours(cx: &Cx<'_>, a: Seat, b: Seat) -> bool {
    if a == b {
        return false;
    }
    if mecatol_neighbours(cx, a, b) {
        return true;
    }
    present_together(cx.adj, &presence(cx.core, a), &presence(cx.core, b))
}

/// `transactions::neighbours`: every seated seat the seat may transact with by the map, in seating
/// order.
#[must_use]
pub fn neighbours(cx: &Cx<'_>, seat: Seat) -> SeatList {
    let mut here: Option<SysSet> = None;
    let mut out = SeatList::new();
    for &other in cx.core.seats().seating_order() {
        if other == seat {
            continue;
        }
        if mecatol_neighbours(cx, seat, other) {
            out.push(other);
            continue;
        }
        let mine = here.get_or_insert_with(|| presence(cx.core, seat));
        if present_together(cx.adj, mine, &presence(cx.core, other)) {
            out.push(other);
        }
    }
    out
}

/// `transactions::neighbours_who_transacted`: neighbours who resolved a transaction this round,
/// each once, in order of first appearance.
#[must_use]
pub fn neighbours_who_transacted(cx: &Cx<'_>, seat: Seat) -> SeatList {
    let mine = neighbours(cx, seat);
    let mut seen = SeatList::new();
    for &(a, b) in cx.core.trade().this_round.iter() {
        for who in [a, b] {
            if who != seat && mine.as_slice().contains(&who) && !seen.as_slice().contains(&who) {
                seen.push(who);
            }
        }
    }
    seen
}

/// `transactions::partners`: the neighbours, or everybody for a seat that reaches past adjacency.
#[must_use]
pub fn partners(cx: &Cx<'_>, seat: Seat) -> SeatList {
    if cx.env.wide(seat) {
        let mut out = SeatList::new();
        for &other in cx.core.seats().seating_order() {
            if other != seat {
                out.push(other);
            }
        }
        return out;
    }
    neighbours(cx, seat)
}

fn seated(core: &CoreState, seat: Seat) -> bool {
    core.seats().seating_order().contains(&seat)
}

/// `transactions::may_transact`: neighbours, or either side able to reach past adjacency.
#[must_use]
pub fn may_transact(cx: &Cx<'_>, proposer: Seat, partner: Seat) -> bool {
    cx.env.transaction_reach(proposer, partner)
        || are_neighbours(cx, proposer, partner)
        || (cx.env.wide(proposer) && partner != proposer && seated(cx.core, partner))
        || (if cx.env.wide(partner) {
            proposer != partner && seated(cx.core, proposer)
        } else {
            seated(cx.core, proposer) && are_neighbours(cx, partner, proposer)
        })
}

// -- the per-turn limits ----------------------------------------------------------------------

/// `GameState::transacted_with(proposer).contains(partner)`.
#[must_use]
pub fn transacted_with(core: &CoreState, proposer: Seat, partner: Seat) -> bool {
    core.trade().transactions_this_turn.get(&proposer).is_some_and(|set| set.contains(partner))
}

/// `transactions::may_open_again` (LRR 94.1).
#[must_use]
pub fn may_open_again(cx: &Cx<'_>, proposer: Seat, partner: Seat) -> bool {
    !transacted_with(cx.core, proposer, partner) || cx.env.exempt_from_limit(proposer, partner)
}

/// `GameState::may_initiate_negotiation`.
#[must_use]
pub fn may_initiate_negotiation(cx: &Cx<'_>, seat: Seat) -> bool {
    let t = cx.core.trade();
    t.negotiations_this_action.get(&seat).copied().unwrap_or(0) < cx.limits.per_action
        && t.negotiations_this_round.get(&seat).copied().unwrap_or(0) < cx.limits.per_round
}

/// The partners `transactions::available_actions` offers to open a transaction with, in seating
/// order (the labels are the adapter's).
#[must_use]
pub fn open_partners(cx: &Cx<'_>, seat: Seat) -> SeatList {
    let mut out = SeatList::new();
    if cx.core.trade().diplomacy.enabled || !may_initiate_negotiation(cx, seat) {
        return out;
    }
    for &other in cx.core.seats().seating_order() {
        if other != seat && may_transact(cx, seat, other) && may_open_again(cx, seat, other) {
            out.push(other);
        }
    }
    out
}

// -- deals ------------------------------------------------------------------------------------

/// Facts about a promissory note in a deal that the core cannot see (notes are string-engine
/// content): computed by the string engine for exactly this deal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoteFacts {
    /// The giver holds it and may lend it (`can_pay`'s note leg).
    pub payable: bool,
    /// The receiver may take it (`promissory::may_receive`).
    pub receivable: bool,
    /// What moves when the deal settles (`None`: an id the tables do not know; the string engine moves it).
    pub mv: Option<NoteMove>,
}

/// The typed form of the note a deal moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteMove {
    /// A note of the table: the holder changes (`promissory::take`).
    Held(crate::state::NoteKey),
    /// Support for the Throne of this faction: the loan and its point (`promissory::receive`).
    Support(crate::generated::Faction),
}

/// The planet-card leg of a deal (Pride of Kenara): whether the giver can pay it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetFacts {
    pub payable: bool,
}

/// What one side of a deal hands over (`transactions::Terms`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Terms {
    pub trade_goods: i32,
    pub commodities: i32,
    /// Relic fragments wanted, by kind (indexed by `ExploreType as usize`).
    pub fragments: [i32; 4],
    pub note: Option<NoteFacts>,
    pub action_card: Option<ActionCard>,
    pub secret: Option<SecretObjective>,
    pub planet: Option<PlanetFacts>,
}

impl Terms {
    /// `Terms::is_empty`: nothing changes hands.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.trade_goods == 0
            && self.commodities == 0
            && self.fragments.iter().all(|n| *n == 0)
            && self.note.is_none()
            && self.action_card.is_none()
            && self.secret.is_none()
            && self.planet.is_none()
    }
}

/// A deal between two seats (`transactions::Offer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub proposer: Seat,
    pub partner: Seat,
    pub given: Terms,
    pub received: Terms,
}

/// Which leg of the deal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leg {
    Given,
    Received,
}

/// `transactions::OfferError`, with seats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Illegal {
    SamePlayer,
    PlayerMissing(Seat),
    Empty,
    NotNeighbours(Seat, Seat),
    CannotPay(Seat),
    /// The receiver of this leg's note may not take it.
    NoteRefused(Seat, Leg),
    ActionCardsNotTradeable,
    MissingActionCard(Seat, ActionCard),
}

fn fragment_held(core: &CoreState, seat: Seat, kind: ExploreType) -> i32 {
    core.players().by_seat(seat).and_then(|p| p.cards.fragments.get(kind)).map_or(0, i32::from)
}

/// `transactions::can_pay`: whether the seat holds what it offers.
#[must_use]
pub fn can_pay(core: &CoreState, seat: Seat, terms: &Terms) -> bool {
    let Some(p) = core.players().by_seat(seat) else {
        return false;
    };
    if terms.trade_goods > i32::from(p.econ.trade_goods) || terms.commodities > i32::from(p.econ.commodities) {
        return false;
    }
    if terms.note.is_some_and(|n| !n.payable) {
        return false;
    }
    if terms.planet.is_some_and(|n| !n.payable) {
        return false;
    }
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        let need = terms.fragments[kind as usize];
        if need > 0 && fragment_held(core, seat, kind) < need {
            return false;
        }
    }
    if let Some(secret) = terms.secret
        && !p.cards.secrets.as_slice().contains(&secret)
    {
        return false;
    }
    true
}

/// `transactions::why_illegal`: why an offer cannot be resolved, or `None`.
#[must_use]
pub fn why_illegal(cx: &Cx<'_>, offer: &Offer) -> Option<Illegal> {
    let core = cx.core;
    if offer.proposer == offer.partner {
        return Some(Illegal::SamePlayer);
    }
    for seat in [offer.proposer, offer.partner] {
        if core.players().by_seat(seat).is_none() {
            return Some(Illegal::PlayerMissing(seat));
        }
    }
    if offer.given.is_empty() && offer.received.is_empty() {
        return Some(Illegal::Empty);
    }
    if core.meta().phase != Phase::Agenda && !may_transact(cx, offer.proposer, offer.partner) {
        return Some(Illegal::NotNeighbours(offer.proposer, offer.partner));
    }
    if !can_pay(core, offer.proposer, &offer.given) {
        return Some(Illegal::CannotPay(offer.proposer));
    }
    if !can_pay(core, offer.partner, &offer.received) {
        return Some(Illegal::CannotPay(offer.partner));
    }
    for (receiver, terms, leg) in [(offer.partner, &offer.given, Leg::Given), (offer.proposer, &offer.received, Leg::Received)] {
        if terms.note.is_some_and(|n| !n.receivable) {
            return Some(Illegal::NoteRefused(receiver, leg));
        }
    }
    if offer.given.action_card.is_some() || offer.received.action_card.is_some() {
        let black_market = cx.env.black_market();
        let arbiters = black_market || cx.env.trades_action_cards(offer.proposer) || cx.env.trades_action_cards(offer.partner);
        if !arbiters {
            return Some(Illegal::ActionCardsNotTradeable);
        }
        for (side, terms) in [(offer.proposer, &offer.given), (offer.partner, &offer.received)] {
            if let Some(card) = terms.action_card
                && !core.players().by_seat(side).is_some_and(|p| p.cards.hand.as_slice().contains(&card))
            {
                return Some(Illegal::MissingActionCard(side, card));
            }
        }
    }
    None
}
