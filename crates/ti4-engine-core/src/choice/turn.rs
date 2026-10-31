//! Decision types of the `turn` area: the action menu of a turn and the closing menu that follows an action (package D1b2;
//! `Game::{turn_options, closing_options}`). The sources that are not ported (transactions, contacts, relics, exploration,
//! faction and mapped component actions, action cards) are LENT AS READS: their options come as
//! [`Opt::Lent`], the code of the option the string engine lists, and the adapter renders them from that list.

use crate::generated::{ActionCard, Explore, ExploreType, Faction, Leader, Relic, StrategyCard, System, Technology};
use crate::ids::Seat;
use crate::state::Code;

/// A component action a faction module offers and later claims. The string engine builds
/// `ChoiceOption`s whose id starts `faction|<alias>|`; the typed form names the module (the offering
/// faction) and a module-defined code with one argument. The adapter renders the id from a table the
/// module registers (C shards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentAction {
    pub module: Faction,
    pub code: u16,
    pub arg: u16,
}

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

impl Opt {
    /// The option as one number (package KM2): the payload of the job that runs it (`TurnJob::Action`) and the entry of the failed
    /// component actions. Injective over the options a live menu lists; `None` for [`Opt::Lent`] (the string's) and [`Opt::Token`] (an
    /// answer to Minister of War's question, not a menu option).
    #[must_use]
    pub fn key(self) -> Option<u32> {
        let tag = |t: u32, payload: u32| Some(t << 24 | payload);
        match self {
            Self::Pass => tag(0, 0),
            Self::Tactical => tag(1, 0),
            Self::Strategic { card, bare } => tag(2, (card.index() as u32) << 1 | u32::from(bare)),
            Self::EndTurn => tag(3, 0),
            Self::Leader(l) => tag(4, l.index() as u32),
            Self::Borrowed(l) => tag(5, l.index() as u32),
            Self::Tech(t) => tag(6, t.index() as u32),
            Self::Purge(k) => tag(7, k as u32),
            Self::UseRelic(r) => tag(8, r.index() as u32),
            Self::Device(c) => tag(9, c.index() as u32),
            Self::Play { index, card } => tag(10, u32::from(index) << 12 | card.index() as u32),
            Self::Salvaged(c) => tag(11, c.index() as u32),
            Self::Open { with, faction } => tag(12, u32::from(with.0) << 8 | faction.index() as u32),
            Self::Faction(a) => tag(13, (a.module.index() as u32) << 16 | u32::from(a.code) << 9 | u32::from(a.arg & 0x1ff)),
            Self::Lent(_) | Self::Token(_) => None,
        }
    }

    /// The option [`Self::key`] names.
    #[must_use]
    pub fn from_key(key: u32) -> Option<Self> {
        let (tag, p) = (key >> 24, key & 0x00ff_ffff);
        let at = p as usize;
        Some(match tag {
            0 => Self::Pass,
            1 => Self::Tactical,
            2 => Self::Strategic { card: StrategyCard::from_index((p >> 1) as usize)?, bare: p & 1 == 1 },
            3 => Self::EndTurn,
            4 => Self::Leader(Leader::from_index(at)?),
            5 => Self::Borrowed(Leader::from_index(at)?),
            6 => Self::Tech(Technology::from_index(at)?),
            7 => Self::Purge(ExploreType::ALL.get(at).copied()?),
            8 => Self::UseRelic(Relic::from_index(at)?),
            9 => Self::Device(Explore::from_index(at)?),
            10 => Self::Play { index: u8::try_from(p >> 12).ok()?, card: ActionCard::from_index((p & 0xfff) as usize)? },
            11 => Self::Salvaged(ActionCard::from_index(at)?),
            12 => Self::Open { with: Seat(u8::try_from(p >> 8).ok()?), faction: Faction::from_index((p & 0xff) as usize)? },
            13 => Self::Faction(ComponentAction { module: Faction::from_index((p >> 16) as usize)?, code: u16::try_from((p >> 9) & 0x7f).ok()?, arg: u16::try_from(p & 0x1ff).ok()? }),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_option_round_trips_through_its_key() {
        let opts = [
            Opt::Pass,
            Opt::Tactical,
            Opt::EndTurn,
            Opt::Strategic { card: StrategyCard::ALL[3], bare: true },
            Opt::Strategic { card: StrategyCard::ALL[3], bare: false },
            Opt::Leader(Leader::ALL[Leader::ALL.len() - 1]),
            Opt::Borrowed(Leader::ALL[5]),
            Opt::Tech(Technology::ALL[Technology::ALL.len() - 1]),
            Opt::Purge(ExploreType::Hazardous),
            Opt::UseRelic(Relic::ALL[Relic::ALL.len() - 1]),
            Opt::Device(Explore::ALL[Explore::ALL.len() - 1]),
            Opt::Play { index: 9, card: ActionCard::ALL[ActionCard::ALL.len() - 1] },
            Opt::Salvaged(ActionCard::ALL[7]),
            Opt::Open { with: Seat(5), faction: Faction::ALL[Faction::ALL.len() - 1] },
            Opt::Faction(ComponentAction { module: Faction::ALL[20], code: 62, arg: 5 }),
        ];
        let mut seen = std::collections::HashSet::new();
        for o in opts {
            let key = o.key().expect("a menu option has a key");
            assert_eq!(Opt::from_key(key), Some(o), "{o:?}");
            assert!(seen.insert(key), "keys are distinct: {o:?}");
        }
        assert_eq!(Opt::Lent(Code(1)).key(), None);
        assert_eq!(Opt::Token(System::ALL[0]).key(), None);
        assert_eq!(Opt::from_key(0xff00_0000), None);
    }
}
