use crate::containers::SmallList;
use crate::generated::{ActionCard, StrategyCard};
use crate::ids::Seat;

use crate::generated::Technology;

/// `ti4_model::state::TransientFlags` (same bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransientFlags(pub u8);

impl TransientFlags {
    pub const AGENDA_DISCARDED: u8 = 1;
    pub const STRATEGIC_CANCELLED: u8 = 1 << 1;
    pub const SKIP_NEXT_TURN: u8 = 1 << 2;
    pub const ADDITIONAL_ACTION: u8 = 1 << 3;
    pub const PUPPET_ACTION: u8 = 1 << 4;
    pub const BLACK_MARKET: u8 = 1 << 5;
    pub const EXPEDITION_CLAIMED: u8 = 1 << 6;

    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

/// A listed starting technologies, in the order the faction sheet names them (order is observable:
/// option order). At most [`TechList::CAP`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechList {
    len: u8,
    items: [Technology; TechList::CAP],
}

impl TechList {
    pub const CAP: usize = 24;

    /// `None` when the list is longer than [`Self::CAP`].
    #[must_use]
    pub fn from_slice(list: &[Technology]) -> Option<Self> {
        if list.len() > Self::CAP {
            return None;
        }
        let mut items = [Technology::ALL[0]; Self::CAP];
        items[..list.len()].copy_from_slice(list);
        Some(Self { len: list.len() as u8, items })
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Technology] {
        &self.items[..usize::from(self.len)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartingTechSource {
    Listed(TechList),
    OwnedByOthers,
    Researchable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingTechPick {
    pub player: Seat,
    pub of: u8,
    pub remaining: u8,
    pub source: StartingTechSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MiscScratch {
    pub flags: TransientFlags,
    pub action_card_plays: SmallList<(Seat, ActionCard), 4>,
    pub declared_card_plays: SmallList<(Seat, ActionCard), 4>,
    pub last_strategy_choice: Option<(Seat, StrategyCard)>,
    pub starting_tech_picks: SmallList<StartingTechPick, 2>,
}
