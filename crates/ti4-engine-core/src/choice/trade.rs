//! Decision types of the `trade` area: the two questions of a transaction window (LRR 94.1a).
//! Package B4. Adapter side: `ti4-engine/src/render/trade.rs`.
//!
//! * [`Ask::Propose`]: the proposer picks a deal. The options are the deals the core derives
//!   (`rules::trade_window::deals`, each a [`Shape`] with the proposer's and the partner's prices computed from
//!   it, `query::trade_deals::nets`), then the generic decline. The question accepts a free-form deal; the core
//!   does not (the free-form id is text, the core has none): the adapter keeps a window with one on the string.
//! * [`Ask::Answer`]: the partner answers the deal on the table: accept (its net worth is the deal's), refuse,
//!   or counter.
//!
//! The "open a transaction" options live in the turn question of the tactical area and stay there.

use crate::generated::{ActionCard, ExploreType, Faction, Planet, SecretObjective};
use crate::ids::Seat;
use crate::state::NoteKey;

/// One deal of the option list, with everything its id, label and payload say (`transactions::offer_options`):
/// the string engine writes the shape once per id, and so does the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    /// `ss`: Support for the Throne swapped (the proposer's copy for the partner's).
    SupportSwap { from: Faction, to: Faction },
    /// `pn{note}:{price}`: a note the proposer holds, sold for `price` trade goods; price 0 is the gift offered to
    /// a partner who cannot pay.
    Sell { note: NoteKey, price: u8 },
    /// `ac{card}:1`: the first action card of the hand for one trade good (Arbiters, Black Market Dealings).
    SellCard { card: ActionCard },
    /// `cn{card}>{note}`: that card for a note of the partner.
    CardForNote { card: ActionCard, note: NoteKey },
    /// `so{secret}:1` (Black Market Dealings).
    SellSecret { secret: SecretObjective },
    /// `fr{kind}:1` (Black Market Dealings).
    SellFragment { kind: ExploreType },
    /// `cc{n}`: `n` commodities each (both gain).
    Swap { n: u16 },
    /// `cc1`: one commodity each (the shorter label of the second line).
    SwapOne,
    /// `ct{give}:{want}`: commodities for trade goods.
    CommoditiesForGoods { give: u16, want: u16 },
    /// `tc{give}:{want}`: trade goods for commodities.
    GoodsForCommodities { give: u16, want: u16 },
    /// `{give}:{want}`: trade goods for trade goods.
    Goods { give: u16, want: u16 },
    /// `c{n}:0`: a gift of commodities.
    GiftCommodities { n: u16 },
    /// `np{note}:{price}`: trade goods for a note of the partner.
    BuyNote { note: NoteKey, price: u8 },
    /// `cp{note}:{price}`: commodities for a note of the partner.
    BuyNoteWithCommodities { note: NoteKey, price: u8 },
    /// `pc{note}:{paid}`: a note of the proposer for `paid` commodities of the partner.
    NoteForCommodities { note: NoteKey, paid: u8 },
    /// `nn{owed}>{want}`: a note for a note.
    NoteForNote { owed: NoteKey, want: NoteKey },
    /// `kp{planet}>{to}:{price}` (Pride of Kenara): the proposer's planet card, their units moving to their planet
    /// `to`, for `price` of the partner's trade goods (0: a gift). `value` is the card's worth (resources and
    /// influence; not in the id).
    PlanetSale { planet: Planet, to: Planet, price: u8, value: u8 },
    /// `kq{planet}>{to}:{price}`: the partner's planet card (their units moving to their planet `to`) for `price` of
    /// the proposer's trade goods.
    PlanetBuy { planet: Planet, to: Planet, price: u8, value: u8 },
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// `proposer` chooses what to put on the table for `partner` (`name`: the partner's faction, which names it).
    Propose { proposer: Seat, partner: Seat, name: Faction },
    /// `responder` answers `deal` of `proposer` (`from`: the proposer's faction, which names them: "X gives A for
    /// B"). The net worth the accept option reports is the deal's (`query::trade_deals::nets`).
    Answer { responder: Seat, proposer: Seat, from: Faction, deal: Shape },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A priced deal.
    Deal(Shape),
    /// Accept (its net worth is in the ask).
    Accept,
    Refuse,
    Counter,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    Propose,
    Answer,
}
