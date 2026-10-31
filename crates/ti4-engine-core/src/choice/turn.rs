//! Decision types of the `turn` area: the action menu of a turn and the closing menu that follows an action (package D1b2;
//! `Game::{turn_options, closing_options}`). The sources that are not ported (transactions, contacts, relics, exploration,
//! faction and mapped component actions, action cards) are LENT AS READS: their options come as
//! [`Opt::Lent`], the code of the option the string engine lists, and the adapter renders them from that list.

use crate::generated::{ActionCard, Explore, ExploreType, Faction, Leader, Relic, StrategyCard, System, Technology};
use crate::ids::Seat;
use crate::kernel::hooks::sites::ComponentAction;
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
    /// A leader's component action: `component|leader|<id>` (package F2: the Leaders source of the menu is the core's).
    Leader(Leader),
    /// A readied Ssruu's copy of another seat's agent: `component|leader|yssarilagent|<agent>`.
    Borrowed(Leader),
    /// A technology's component action: `component|tech|<alias>` (Sling Relay is the one the engine implements).
    Tech(Technology),
    /// Purge three relic fragments of a trait (frontier fragments stand in) for a relic: `purge|<TRAIT>` (package KM0).
    Purge(ExploreType),
    /// Use a relic in the play area: `relic|<alias>`.
    UseRelic(Relic),
    /// An Enigmatic Device exploration card in the play area: `play_area:<card>`.
    Device(Explore),
    /// An action card of the hand played as a component action: `action_card|<index>` (the card is in the option so the label needs no hand).
    Play { index: u8, card: ActionCard },
    /// An action card off the salvage yard (Garbozia): `action_card|salvaged|<alias>`.
    Salvaged(ActionCard),
    /// Open a transaction with a neighbour: `component|trade|<faction>`.
    Open { with: Seat, faction: Faction },
    /// A faction's own component action (Orbital Drop, Production Biomes, Trade Convoys and the faction modules'): `faction|...`; the ids and
    /// labels are the renderer's table over `(module, code, arg)`.
    Faction(ComponentAction),
}
