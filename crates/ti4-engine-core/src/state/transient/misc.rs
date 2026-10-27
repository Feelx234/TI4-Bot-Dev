use crate::containers::SmallList;
use crate::generated::{ActionCard, StrategyCard};
use crate::ids::Seat;

use super::super::player::TechSet;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartingTechSource {
    Listed(TechSet),
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
