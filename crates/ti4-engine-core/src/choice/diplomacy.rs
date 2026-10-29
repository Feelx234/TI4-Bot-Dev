//! Decision types of the `diplomacy` area: the structured negotiation window (LRR 94, rules v1). Package B4d.
//! Adapter side: `ti4-engine/src/render/diplomacy.rs`.
//!
//! One question builds a revision item by item ([`Ask::Build`]), one asks how many ([`Ask::Amount`]), one
//! reviews the terms ([`Ask::Review`]), one answers the revision on the table ([`Ask::Respond`]). The options
//! are derived by the window (`query::diplomacy_items`), never stored; an item is a short code ([`Item`]), the
//! term it adds is built from it with the amount and the round. The draft itself (the terms so far) lives in the
//! window, not in the question: the question names who builds and where the build stands.
//!
//! Size: `Opt` stays within 16 bytes (`choice::tests::opt_stays_small`) because a term is not stored in an
//! option: a pledge carries only what the player picks (a seat, a system, an agenda and outcome, a leader) and
//! a one-bit deadline (this round or next), the other fields of the term follow from the round.

use crate::generated::{ActionCard, Agenda, Leader, System};
use crate::ids::Seat;
use crate::keys::Outcome;
use crate::state::NoteKey;

/// A component that comes in amounts (`builder::Counted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Counted {
    TradeGoods,
    Commodities,
    Cultural,
    Hazardous,
    Industrial,
    Unknown,
}

impl Counted {
    pub const ALL: [Self; 6] = [Self::TradeGoods, Self::Commodities, Self::Cultural, Self::Hazardous, Self::Industrial, Self::Unknown];
}

/// A promise that is not a payment. `late`: the deadline is the next round, not this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pledge {
    /// `DoNotAttack` of the other side of the item.
    NoAttack { late: bool },
    NoActivate { system: System, late: bool },
    /// `Attack` a third seat.
    Attack { who: Seat, late: bool },
    /// `Vote` on the agenda under negotiation (deadline: this round).
    Vote { agenda: Agenda, outcome: Outcome },
    /// `ReplenishFor` the other side (deadline: next round).
    Replenish,
    /// `UseLeaderFor` the other side (deadline: this round).
    Leader { leader: Leader },
}

/// Something the builder can add to the side being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Item {
    /// A counted component handed over now (the amount follows).
    Now(Counted),
    /// A counted payment promised for the next round (the amount follows).
    Later(Counted),
    Note(NoteKey),
    Card(ActionCard),
    Pledge(Pledge),
}

/// Where a build stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// Adding to what the builder gives.
    Give,
    /// Adding to what the builder asks for.
    Take,
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// `builder` picks the next item of the revision for `other`.
    Build { builder: Seat, other: Seat, step: Step },
    /// `builder` says how many of `kind` (`later`: promised for next round).
    Amount { builder: Seat, other: Seat, kind: Counted, later: bool },
    /// `builder` reviews the terms: propose, edit or walk away (`back`: a counter on the table to return to).
    Review { builder: Seat, other: Seat, back: bool },
    /// `actor` answers the revision of `other` (`may_counter`: a counter is still allowed).
    Respond { actor: Seat, other: Seat, may_counter: bool },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    Item(Item),
    /// An amount of the pending item.
    Amount(u8),
    /// Remove the term at this index of the side being edited.
    Remove(u8),
    /// Done offering / done asking.
    Done,
    Propose,
    Edit,
    /// Make no offer / back to the offer on the table.
    Cancel,
    /// Send the n-th signal of the contact instead of an offer.
    Signal(u8),
    Accept,
    Decline,
    Counter,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    OfferItem,
    AskItem,
    Amount,
    Review,
    Response,
}
