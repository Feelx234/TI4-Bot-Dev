//! Decision types of the `turn` area: the action menu of a turn and the closing menu that follows an action (package D1b2;
//! `Game::{turn_options, closing_options}`). The sources that are not ported (transactions, contacts, relics, exploration,
//! technologies, faction and mapped component actions, leaders, action cards) are LENT AS READS: their options come as
//! [`Opt::Lent`], the code of the option the string engine lists, and the adapter renders them from that list.

use crate::generated::{StrategyCard, System};
use crate::state::Code;

/// What the area asks. Parameters only; the option list is assembled by `driver::menu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// The action menu: "action phase".
    ActionMenu,
    /// The closing menu after an action: "end your turn", or with `second` "end your turn, or take your second action".
    Closing { second: bool },
    /// Minister of War: "which command token comes back".
    MinisterToken,
}

/// The machine identifier of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    ActionMenu,
    EndTurn,
    MinisterToken,
}

/// One answer the area offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// "pass".
    Pass,
    /// "take a tactical action".
    Tactical,
    /// A strategic action: `bare` is the lone unused card's id (`strategic`), else `strategic|<card>`.
    Strategic { card: StrategyCard, bare: bool },
    /// "end your turn".
    EndTurn,
    /// An option of a source that is not ported: the interned id of the option the string lists (the adapter's table holds the
    /// option itself).
    Lent(Code),
    /// Minister of War: retrieve the command token from this system.
    Token(System),
}
