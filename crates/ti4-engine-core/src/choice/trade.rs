//! Decision types of the `trade` area: the two questions of a transaction window (LRR 94.1a).
//! Package B4. Adapter side: `ti4-engine/src/render/trade.rs`.
//!
//! * [`Ask::Propose`]: the proposer picks a deal. The options are the deals `offer_options` priced
//!   ([`Opt::Deal`], an interned shape: id, kind, label and payload stay the string engine's until
//!   the pricing is ported), then the generic decline. The question accepts a free-form deal.
//! * [`Ask::Answer`]: the partner answers the deal on the table: accept (carrying its net worth),
//!   refuse, or counter.
//!
//! The "open a transaction" options live in the turn question of the tactical area and stay there.

use crate::ids::Seat;
use crate::state::Code;

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// `proposer` chooses what to put on the table for `partner` (`name`: the partner's faction name).
    Propose { proposer: Seat, partner: Seat, name: Code },
    /// `responder` answers a deal of `proposer` (`said`: the deal read aloud, "X gives A for B"; `net`: the exact text of the
    /// net worth the accept option reports).
    Answer { responder: Seat, proposer: Seat, said: Code, net: Code },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A priced deal, as an interned option shape.
    Deal(Code),
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
