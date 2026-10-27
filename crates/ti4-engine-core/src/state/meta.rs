//! `Meta`: round, phase, speaker, active player, rules version, sequence counters and flags.

use crate::containers::{Opt32, SeatSet};
use crate::generated::{GalacticEvent, System, Wormhole};
use crate::ids::Seat;

/// `ti4_model::state::Phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Phase {
    #[default]
    Strategy,
    Action,
    Status,
    Agenda,
}

impl Phase {
    #[must_use]
    pub const fn uses_speaker_order(self) -> bool {
        matches!(self, Self::Strategy | Self::Agenda)
    }
}

/// Single-bit game flags (`GameState` bools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MetaFlags(pub u8);

impl MetaFlags {
    pub const LEGACY_DEVICE_IDS: u8 = 1;
    pub const CUSTODIANS_REMOVED: u8 = 1 << 1;
    pub const FRACTURE_IN_PLAY: u8 = 1 << 2;
    pub const NEXUS_UNLOCKED: u8 = 1 << 3;
    pub const THUNDERS_EDGE: u8 = 1 << 4;

    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    pub fn set(&mut self, flag: u8, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meta {
    /// `GameState.rules_version`, read by every gated rule where the string engine reads it.
    pub rules_version: u32,
    pub round: u32,
    pub phase: Phase,
    pub speaker: Seat,
    pub active: Option<Seat>,
    pub active_system: Option<System>,
    pub finished: bool,
    pub flags: MetaFlags,
    /// `GameState.strategy_cards_per_player` (a count of at most a handful).
    pub strategy_cards_per_player: u8,
    pub styx_holder: Option<Seat>,
    pub thunders_edge_system: Option<System>,
    pub galactic_event: Option<GalacticEvent>,
    /// `GameState.ion_storm`: the system and the face (the wormhole kind it shows).
    pub ion_storm: Option<(System, Wormhole)>,
    /// `GameState.influence_pays_for_units`.
    pub influence_pays_for_units: SeatSet,
    // -- sequence counters ----------------------------------------------------------------
    pub combat_round_seq: u32,
    pub production_seq: u32,
    pub activation_seq: u32,
    pub agenda_seq: u32,
    pub turn_seq: u32,
    pub action_turn: u32,
    pub feat_occurrence_seq: u64,
    pub combat_draw_round: Opt32,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            rules_version: 0,
            round: 0,
            phase: Phase::Strategy,
            speaker: Seat(0),
            active: None,
            active_system: None,
            finished: false,
            flags: MetaFlags(0),
            strategy_cards_per_player: 0,
            styx_holder: None,
            thunders_edge_system: None,
            galactic_event: None,
            ion_storm: None,
            influence_pays_for_units: SeatSet::EMPTY,
            combat_round_seq: 0,
            production_seq: 0,
            activation_seq: 0,
            agenda_seq: 0,
            turn_seq: 0,
            action_turn: 0,
            feat_occurrence_seq: 0,
            combat_draw_round: Opt32::NONE,
        }
    }
}

impl Meta {
    /// Whether the game runs at least `version` (the gated-rule test).
    #[must_use]
    pub const fn rules_at_least(&self, version: u32) -> bool {
        self.rules_version >= version
    }
}
