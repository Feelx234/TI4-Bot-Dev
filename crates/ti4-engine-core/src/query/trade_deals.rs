//! The deals a seat can put on the table and what they are worth (`transactions::offer_options`, `Terms::
//! worth_to_receiver`, `Terms::cost_to_giver`, `note_worth`, `note_cost`, `note_option_price`).
//!
//! Package B4c. The string engine writes every deal as a shape (`id`, `label`, `payload`) and prices it from the deal
//! its id names; here a deal is a [`Shape`] (`choice::trade`), the list is [`deals`] and the prices are [`nets`].
//! The prices are the string engine's `f64` sums, taken in the same order, because they are printed (a payload
//! number, the accept option's net): `(((tg + c) + frag) + note) + card) + secret`.
//!
//! What the core cannot see comes in as [`DealFacts`]: the order of note ids and of card aliases as the text sorts
//! (the string maps are ordered by text, the typed ones by enum), the leaders (an Alliance conveys nothing until its
//! owner's commander is unlocked) and the hooks (who may deal with whom, who may trade action cards). They are the
//! string engine's answers, computed once for the window (the faction modules are not ported yet).
//!
//! Not here: the Pride of Kenara planet cards (`kp`/`kq` shapes). A seat with a mech on a planet it controls has
//! them, and the adapter keeps such a window on the string.

use core::cmp::Ordering;

use crate::generated::{ActionCard, ExploreType, Faction, Planet, PromissoryNote, Unit};
use crate::choice::trade::Shape;
use crate::ids::Seat;
use crate::query::promissory as qn;
use crate::query::trade::{NoteFacts, NoteMove, Offer, Terms};
use crate::state::{CoreState, NoteKey, TransientFlags};

/// The answers of the string engine a window needs (see the module docs).
#[derive(Clone, Copy)]
pub struct DealFacts {
    /// `BTreeMap<String, _>` order of note ids: the text of `{alias}:{faction}`.
    pub note_order: fn(NoteKey, NoteKey) -> Ordering,
    /// `ActionCardId` order: the text of the alias.
    pub card_order: fn(ActionCard, ActionCard) -> Ordering,
    /// Bit `seat` is set when the seat's commander is unlocked (`promissory::commander_unlocked`).
    pub unlocked: u16,
    /// The two chairs the next three facts are about (the window's proposer and partner when it was built).
    pub pair: (Seat, Seat),
    /// `transactions::may_transact(pair.0, pair.1)` and `(pair.1, pair.0)`.
    pub may: [bool; 2],
    /// `transactions::trades_action_cards` of `pair.0` and of `pair.1` (Arbiters).
    pub arbiters: [bool; 2],
    /// The worth of each planet card, indexed by `Planet::index` (`planet_value`: resources plus influence, at
    /// least one; two for a planet the content does not know). Read by the Pride of Kenara deals only.
    pub planet_values: &'static [u8],
}

impl DealFacts {
    /// Whether `from` may deal with the other chair (`may_transact(from, other)`).
    #[must_use]
    pub fn may_from(&self, from: Seat) -> bool {
        self.may[usize::from(from != self.pair.0)]
    }

    /// Whether the seat trades action cards (Arbiters).
    #[must_use]
    pub fn arbiter(&self, seat: Seat) -> bool {
        self.arbiters[usize::from(seat != self.pair.0)]
    }
}

impl core::fmt::Debug for DealFacts {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DealFacts").field("unlocked", &self.unlocked).field("pair", &self.pair).field("may", &self.may).field("arbiters", &self.arbiters).finish()
    }
}

impl PartialEq for DealFacts {
    fn eq(&self, other: &Self) -> bool {
        self.unlocked == other.unlocked && self.pair == other.pair && self.may == other.may && self.arbiters == other.arbiters
    }
}

// -- prices -----------------------------------------------------------------------------------

/// What a leg of a deal moves, for the price: a note of the table, or a Support for the Throne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Note(NoteKey),
    Support,
}

/// One side of a deal as `Terms::worth_to_receiver` and `cost_to_giver` see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Side {
    pub goods: i32,
    pub commodities: i32,
    pub fragments: i32,
    pub note: Option<Held>,
    pub card: bool,
    pub secret: bool,
    /// A planet card and its worth (Pride of Kenara).
    pub planet: Option<u8>,
}

/// `transactions::PLANET_GIVING_COST`: more than any deal pays.
pub const PLANET_GIVING_COST: f64 = 100.0;

/// `promissory::trade_agreement_worth`: slightly under the owner's commodity value.
#[must_use]
pub fn trade_agreement_worth(owner: Faction) -> f64 {
    0.8 * f64::from(owner.data().commodities)
}

/// The `NOTE_WORTH` table row of a note (1.5 for a note the table does not name).
#[must_use]
fn table_worth(note: PromissoryNote) -> f64 {
    match note {
        PromissoryNote::ResearchAgreement => 4.0,
        PromissoryNote::Alliance | PromissoryNote::TradeConvoys => 3.0,
        PromissoryNote::TradeAgreement => 2.5,
        PromissoryNote::CyberneticEnhancements | PromissoryNote::MilitarySupport | PromissoryNote::PoliticalFavor | PromissoryNote::WarFunding => 2.0,
        _ => 1.5,
    }
}

/// `note_worth`: Support is 4, a Trade Agreement its live value, the rest the table.
#[must_use]
pub fn note_worth(held: Held) -> f64 {
    match held {
        Held::Support => 4.0,
        Held::Note(key) if key.note == PromissoryNote::TradeAgreement => trade_agreement_worth(key.faction),
        Held::Note(key) => table_worth(key.note),
    }
}

/// `note_cost`: Support gives up a point for 3; any other note costs three quarters of its worth.
#[must_use]
pub fn note_cost(held: Held) -> f64 {
    match held {
        Held::Support => 3.0,
        Held::Note(_) => 0.75 * note_worth(held),
    }
}

/// `note_option_price`: the posted price, from the flat table (a Trade Agreement is 2.5 there, not its live worth).
#[must_use]
pub fn option_price(note: PromissoryNote) -> u8 {
    py_round_half_even(table_worth(note))
}

/// Python's `round` of the table's values (whole numbers and exact halves): halves go to the even neighbour.
fn py_round_half_even(value: f64) -> u8 {
    let floor = value.floor();
    let rounded = if (floor + 0.5 - value).abs() <= 1e-9 {
        if floor % 2.0 == 0.0 { floor } else { floor + 1.0 }
    } else {
        value.round()
    };
    rounded as u8
}

impl Side {
    /// `Terms::worth_to_receiver`.
    #[must_use]
    pub fn worth(&self) -> f64 {
        let note = self.note.map_or(0.0, note_worth);
        f64::from(self.goods)
            + f64::from(self.commodities)
            + f64::from(self.fragments)
            + note
            + f64::from(u8::from(self.card))
            + f64::from(u8::from(self.secret))
            + self.planet.map_or(0.0, f64::from)
    }

    /// `Terms::cost_to_giver`.
    #[must_use]
    pub fn cost(&self) -> f64 {
        let note = self.note.map_or(0.0, note_cost);
        f64::from(self.goods)
            + 0.2 * f64::from(self.commodities)
            + f64::from(self.fragments)
            + note
            + f64::from(u8::from(self.card))
            + f64::from(u8::from(self.secret))
            + self.planet.map_or(0.0, |_| PLANET_GIVING_COST)
    }
}

fn i<T: Into<i32>>(x: T) -> i32 {
    x.into()
}
fn goods(n: i32) -> Side {
    Side { goods: n, ..Side::default() }
}
fn commodities(n: i32) -> Side {
    Side { commodities: n, ..Side::default() }
}
fn note(key: NoteKey) -> Side {
    Side { note: Some(Held::Note(key)), ..Side::default() }
}

/// The deal a shape names: what the proposer gives and what they receive (`offer_from` of its id).
#[must_use]
pub fn sides(shape: Shape) -> (Side, Side) {
    match shape {
        Shape::SupportSwap { .. } => (Side { note: Some(Held::Support), ..Side::default() }, Side { note: Some(Held::Support), ..Side::default() }),
        Shape::Sell { note: n, price } => (note(n), goods(i(price))),
        Shape::SellCard { .. } => (Side { card: true, ..Side::default() }, goods(1)),
        Shape::CardForNote { note: n, .. } => (Side { card: true, ..Side::default() }, note(n)),
        Shape::SellSecret { .. } => (Side { secret: true, ..Side::default() }, goods(1)),
        Shape::SellFragment { .. } => (Side { fragments: 1, ..Side::default() }, goods(1)),
        Shape::Swap { n } => (commodities(i(n)), commodities(i(n))),
        Shape::SwapOne => (commodities(1), commodities(1)),
        Shape::CommoditiesForGoods { give, want } => (commodities(i(give)), goods(i(want))),
        Shape::GoodsForCommodities { give, want } => (goods(i(give)), commodities(i(want))),
        Shape::Goods { give, want } => (goods(i(give)), goods(i(want))),
        Shape::GiftCommodities { n } => (commodities(i(n)), Side::default()),
        Shape::BuyNote { note: n, price } => (goods(i(price)), note(n)),
        Shape::BuyNoteWithCommodities { note: n, price } => (commodities(i(price)), note(n)),
        Shape::NoteForCommodities { note: n, paid } => (note(n), commodities(i(paid))),
        Shape::NoteForNote { owed, want } => (note(owed), note(want)),
        Shape::PlanetSale { price, value, .. } => (Side { planet: Some(value), ..Side::default() }, goods(i(price))),
        Shape::PlanetBuy { price, value, .. } => (goods(i(price)), Side { planet: Some(value), ..Side::default() }),
    }
}

/// `(net, their_net)` of a shape: what the proposer is better off by, and the partner from the other chair
/// (`priced`). The accept option of the partner reports `their_net`.
#[must_use]
pub fn nets(shape: Shape) -> (f64, f64) {
    let (given, received) = sides(shape);
    (received.worth() - given.cost(), given.worth() - received.cost())
}

// -- the list ---------------------------------------------------------------------------------

fn econ(core: &CoreState, seat: Seat) -> (i32, i32) {
    core.players().by_seat(seat).map_or((0, 0), |p| (i32::from(p.econ.trade_goods), i32::from(p.econ.commodities)))
}

fn u16_of(n: i32) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// The seat that plays `faction` first in seating order (`promissory::seat_of`).
fn seat_of(core: &CoreState, faction: Faction) -> Option<Seat> {
    core.seats().seating_order().iter().copied().find(|s| qn::faction_of(core, *s) == Some(faction))
}

/// `promissory::available_notes`: the notes the seat holds that are in hand and convey something, in the text order
/// of the ids.
#[must_use]
pub fn available_notes(core: &CoreState, f: &DealFacts, seat: Seat) -> Vec<NoteKey> {
    let mut notes: Vec<NoteKey> = qn::held_by(core, seat)
        .into_iter()
        .filter(|k| !core.cards().promissory_faceup.contains_key(k))
        .filter(|k| {
            k.note != PromissoryNote::Alliance || seat_of(core, k.faction).is_some_and(|owner| f.unlocked & (1 << owner.0) != 0)
        })
        .collect();
    notes.sort_by(|a, b| (f.note_order)(*a, *b));
    notes
}

/// `promissory::available_support`: the seat's Support for the Throne, while it is not lent.
#[must_use]
pub fn available_support(core: &CoreState, seat: Seat) -> Option<Faction> {
    let faction = qn::faction_of(core, seat)?;
    (!core.trade().support_holders.contains_key(&seat)).then_some(faction)
}

const KINDS: [ExploreType; 4] = [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial];

fn black_market(core: &CoreState) -> bool {
    core.transient().misc.flags.has(TransientFlags::BLACK_MARKET)
}

/// `transactions::offer_options`, as shapes, in the order the string engine lists them.
#[must_use]
pub fn deals(core: &CoreState, f: &DealFacts, proposer: Seat, partner: Seat) -> Vec<Shape> {
    let (mine_goods, mine_commodities) = econ(core, proposer);
    let (their_goods, their_commodities) = econ(core, partner);
    let mut out = Vec::new();

    if let (Some(from), Some(to)) = (available_support(core, proposer), available_support(core, partner)) {
        out.push(Shape::SupportSwap { from, to });
    }

    let mine: Vec<NoteKey> = available_notes(core, f, proposer);
    let theirs: Vec<NoteKey> = available_notes(core, f, partner);
    for key in &mine {
        if !qn::may_receive(core, partner, key.note) {
            continue;
        }
        let price = i32::from(option_price(key.note));
        if price > 0 && their_goods < price {
            out.push(Shape::Sell { note: *key, price: 0 });
        }
        if price > 0 && their_goods >= price {
            out.push(Shape::Sell { note: *key, price: option_price(key.note) });
        }
    }

    // The Arbiters / Black Market door into 94.3: the first card of the hand by alias.
    let card_door = black_market(core) || f.arbiters[0] || f.arbiters[1];
    if card_door
        && let Some(card) = core.players().by_seat(proposer).and_then(|p| p.cards.hand.as_slice().iter().copied().min_by(|a, b| (f.card_order)(*a, *b)))
    {
        if their_goods >= 1 {
            out.push(Shape::SellCard { card });
        }
        for key in theirs.iter().filter(|k| qn::may_receive(core, proposer, k.note)).take(3) {
            out.push(Shape::CardForNote { card, note: *key });
        }
    }

    if black_market(core)
        && let Some(p) = core.players().by_seat(proposer)
    {
        for secret in p.cards.secrets.as_slice() {
            if their_goods >= 1 {
                out.push(Shape::SellSecret { secret: *secret });
            }
        }
        for kind in KINDS {
            if p.cards.fragments.get(kind).is_some_and(|n| n > 0) && their_goods >= 1 {
                out.push(Shape::SellFragment { kind });
            }
        }
    }

    let swap = mine_commodities.min(their_commodities);
    if swap > 0 {
        out.push(Shape::Swap { n: u16_of(swap) });
        if swap > 1 {
            out.push(Shape::SwapOne);
        }
    }
    if mine_commodities > 0 && their_goods > 0 {
        let give = mine_commodities.min(3);
        let want = their_goods.min(give);
        if want > 0 {
            out.push(Shape::CommoditiesForGoods { give: u16_of(give), want: u16_of(want) });
        }
    }
    if their_commodities > 0 && mine_goods > 0 {
        let want = their_commodities.min(3);
        let give = mine_goods.min(want);
        if give > 0 {
            out.push(Shape::GoodsForCommodities { give: u16_of(give), want: u16_of(want) });
        }
    }
    for give in 0..=mine_goods.min(3) {
        for want in 0..=their_goods.min(3) {
            if give == 0 && want == 0 {
                continue;
            }
            out.push(Shape::Goods { give: u16_of(give), want: u16_of(want) });
        }
    }
    if mine_commodities > 0 {
        out.push(Shape::GiftCommodities { n: u16_of(mine_commodities) });
    }

    partner_assets(core, f, proposer, partner, &mine, &theirs, &mut out);
    planet_shapes(core, f, proposer, partner, &mut out);
    out
}

/// `transactions::pride_of_kenara_planets`: the planets the seat could trade away with a Pride of Kenara (rules
/// version 17), each with the planets its units could move to. A planet with a Pride of Kenara of the seat on it,
/// of the planets the seat controls, in system then planet order.
#[must_use]
pub fn kenara_planets(core: &CoreState, seat: Seat) -> Vec<(Planet, Vec<Planet>)> {
    if !core.loaded.has(crate::state::Slice::Board) || core.meta().rules_version < RULES_FACTION_BUGFIXES {
        return Vec::new();
    }
    let owned = crate::query::board::controlled_planets(core, seat);
    owned
        .as_slice()
        .iter()
        .filter(|(_, planet)| core.board().planet(*planet).units.as_slice().iter().any(|u| u.kind() == Unit::PrideOfKenara && u.owner() == seat))
        .filter_map(|(_, planet)| {
            let moves: Vec<Planet> = owned.as_slice().iter().map(|(_, other)| *other).filter(|other| other != planet).collect();
            (!moves.is_empty()).then_some((*planet, moves))
        })
        .collect()
}

/// `transactions::RULES_FACTION_BUGFIXES`: the rules version of the Pride of Kenara's trade.
pub const RULES_FACTION_BUGFIXES: u32 = 17;

/// `planet_shapes`: the planet cards a Pride of Kenara lets change hands, as gifts, sales and asks.
fn planet_shapes(core: &CoreState, f: &DealFacts, proposer: Seat, partner: Seat, out: &mut Vec<Shape>) {
    let (my_goods, _) = econ(core, proposer);
    let (their_goods, _) = econ(core, partner);
    let value = |planet: Planet| f.planet_values.get(planet.index()).copied().unwrap_or(2);
    for (planet, moves) in kenara_planets(core, proposer) {
        for to in moves {
            let price = value(planet);
            out.push(Shape::PlanetSale { planet, to, price: 0, value: price });
            if their_goods >= i32::from(price) {
                out.push(Shape::PlanetSale { planet, to, price, value: price });
            }
        }
    }
    for (planet, moves) in kenara_planets(core, partner) {
        let price = value(planet);
        if my_goods < i32::from(price) {
            continue;
        }
        for to in moves {
            out.push(Shape::PlanetBuy { planet, to, price, value: price });
        }
    }
}

/// `partner_assets`: the asks that take something from the partner.
fn partner_assets(core: &CoreState, f: &DealFacts, proposer: Seat, partner: Seat, mine: &[NoteKey], theirs: &[NoteKey], out: &mut Vec<Shape>) {
    let (my_goods, my_commodities) = econ(core, proposer);
    let (_, their_commodities) = econ(core, partner);
    // Hubris: nobody gives the Mahact player an Alliance.
    let mine: Vec<NoteKey> = mine.iter().copied().filter(|k| qn::may_receive(core, partner, k.note)).collect();
    let theirs: Vec<NoteKey> = theirs.iter().copied().filter(|k| qn::may_receive(core, proposer, k.note)).collect();
    for key in &theirs {
        let price = option_price(key.note);
        let p = i32::from(price);
        if p > 0 && my_goods >= p {
            out.push(Shape::BuyNote { note: *key, price });
        }
        if p > 0 && my_commodities >= p {
            out.push(Shape::BuyNoteWithCommodities { note: *key, price });
        }
    }
    for owed in &mine {
        let paid = their_commodities.min(i32::from(option_price(owed.note)).max(1));
        if paid <= 0 {
            continue;
        }
        out.push(Shape::NoteForCommodities { note: *owed, paid: u8::try_from(paid).unwrap_or(u8::MAX) });
    }
    // Note for note: different promises only, best net first, ties by the cards' ids.
    let mut pairs: Vec<(f64, NoteKey, NoteKey)> = Vec::new();
    for owed in &mine {
        for want in &theirs {
            if want.note == owed.note {
                continue;
            }
            let net = note_worth(Held::Note(*want)) - note_cost(Held::Note(*owed));
            if net >= 0.0 {
                pairs.push((net, *owed, *want));
            }
        }
    }
    pairs.sort_by(|l, r| r.0.total_cmp(&l.0).then_with(|| (f.note_order)(l.1, r.1)).then_with(|| (f.note_order)(l.2, r.2)));
    for (_, owed, want) in pairs.into_iter().take(4) {
        out.push(Shape::NoteForNote { owed, want });
    }
}

// -- the deal as a legality question ----------------------------------------------------------

fn fragments_of(kind: ExploreType) -> [i32; 4] {
    let mut out = [0; 4];
    out[kind as usize] = 1;
    out
}

/// The deal of a shape on the typed state, with the facts of its note legs (`transactions::offer_from` and the
/// adapter's `terms`): `None` for a deal the state cannot name (an owner that two seats play).
#[must_use]
pub fn offer_of(core: &CoreState, f: &DealFacts, shape: Shape, proposer: Seat, partner: Seat) -> Offer {
    let leg = |held: Option<Held>, giver: Seat, receiver: Seat| {
        held.map(|h| match h {
            Held::Note(key) => NoteFacts {
                payable: available_notes(core, f, giver).contains(&key),
                receivable: qn::may_receive(core, receiver, key.note),
                mv: Some(NoteMove::Held(key)),
            },
            Held::Support => NoteFacts { payable: false, receivable: true, mv: None },
        })
    };
    let (g, r) = sides(shape);
    let mut given = Terms { trade_goods: g.goods, commodities: g.commodities, note: leg(g.note, proposer, partner), ..Terms::default() };
    let mut received = Terms { trade_goods: r.goods, commodities: r.commodities, note: leg(r.note, partner, proposer), ..Terms::default() };
    match shape {
        Shape::SupportSwap { from, to } => {
            // Support is a position, not a note of the map: payable while the giver's copy is not lent.
            given.note = Some(NoteFacts { payable: available_support(core, proposer) == Some(from), receivable: true, mv: Some(NoteMove::Support(from)) });
            received.note = Some(NoteFacts { payable: available_support(core, partner) == Some(to), receivable: true, mv: Some(NoteMove::Support(to)) });
        }
        Shape::SellCard { card } | Shape::CardForNote { card, .. } => given.action_card = Some(card),
        Shape::SellSecret { secret } => given.secret = Some(secret),
        Shape::SellFragment { kind } => given.fragments = fragments_of(kind),
        Shape::PlanetSale { planet, to, .. } => given.planet = Some(planet_leg(core, proposer, planet, to)),
        Shape::PlanetBuy { planet, to, .. } => received.planet = Some(planet_leg(core, partner, planet, to)),
        _ => {}
    }
    Offer { proposer, partner, given, received }
}

/// The planet-card leg: whether the giver could pay it now (`planet_term_payable`) and what moves.
fn planet_leg(core: &CoreState, giver: Seat, planet: Planet, to: Planet) -> crate::query::trade::PlanetFacts {
    let payable = kenara_planets(core, giver).iter().any(|(p, moves)| *p == planet && moves.contains(&to));
    crate::query::trade::PlanetFacts { payable, mv: Some((planet, to)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(note: PromissoryNote, faction: Faction) -> NoteKey {
        NoteKey { note, faction }
    }

    #[test]
    fn the_posted_price_rounds_the_worth_half_to_even() {
        assert_eq!(option_price(PromissoryNote::ResearchAgreement), 4);
        assert_eq!(option_price(PromissoryNote::Alliance), 3);
        assert_eq!(option_price(PromissoryNote::TradeConvoys), 3);
        // 2.5 -> 2 (the even neighbour), and the flat row prices a Trade Agreement whatever its owner's commodities.
        assert_eq!(option_price(PromissoryNote::TradeAgreement), 2);
        assert_eq!(option_price(PromissoryNote::Ceasefire), 2);
        assert_eq!(option_price(PromissoryNote::PoliticalSecret), 2);
        assert_eq!(option_price(PromissoryNote::Stymie), 2, "an unnamed note is 1.5, which rounds to 2");
        assert_eq!(option_price(PromissoryNote::MilitarySupport), 2);
    }

    #[test]
    fn a_note_is_worth_its_row_and_costs_three_quarters_of_it() {
        let held = |n| Held::Note(key(n, Faction::TheEmiratesOfHacan));
        assert_eq!(note_worth(held(PromissoryNote::ResearchAgreement)), 4.0);
        assert_eq!(note_worth(held(PromissoryNote::Ceasefire)), 1.5);
        assert_eq!(note_worth(Held::Support), 4.0);
        assert_eq!(note_cost(Held::Support), 3.0);
        assert_eq!(note_cost(held(PromissoryNote::ResearchAgreement)), 3.0);
        assert_eq!(note_cost(held(PromissoryNote::Ceasefire)), 0.75 * 1.5);
        // A Trade Agreement is worth under its owner's commodity value (Hacan: 6).
        assert_eq!(note_worth(held(PromissoryNote::TradeAgreement)), 0.8 * 6.0);
    }

    #[test]
    fn the_nets_are_the_strings_sums_in_the_strings_order() {
        // Goods for goods: what comes in against what goes out.
        assert_eq!(nets(Shape::Goods { give: 1, want: 2 }), (1.0, -1.0));
        // A commodity costs a fifth to give and is worth a whole good to receive (`0.2 * 3.0` is not 0.6).
        let (net, theirs) = nets(Shape::CommoditiesForGoods { give: 3, want: 2 });
        assert_eq!(net, 2.0 - 0.2 * 3.0);
        assert_eq!(theirs, 3.0 - 2.0);
        assert_eq!(nets(Shape::Swap { n: 2 }), (2.0 - 0.2 * 2.0, 2.0 - 0.2 * 2.0));
        assert_eq!(nets(Shape::GiftCommodities { n: 4 }), (0.0 - 0.2 * 4.0, 4.0 - 0.0));
        // A card and a secret price flat at one in both directions; a fragment likewise.
        assert_eq!(nets(Shape::SellFragment { kind: ExploreType::Cultural }), (1.0 - 1.0, 1.0 - 1.0));
        let note = key(PromissoryNote::ResearchAgreement, Faction::TheEmiratesOfHacan);
        assert_eq!(nets(Shape::Sell { note, price: 4 }), (4.0 - 3.0, 4.0 - 4.0));
        assert_eq!(nets(Shape::Sell { note, price: 0 }), (0.0 - 3.0, 4.0 - 0.0));
        assert_eq!(nets(Shape::SupportSwap { from: Faction::TheEmiratesOfHacan, to: Faction::TheEmiratesOfHacan }), (4.0 - 3.0, 4.0 - 3.0));
    }
}
