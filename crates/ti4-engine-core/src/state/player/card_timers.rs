use super::timers::ExtraVotes;
use crate::containers::Opt32;
use crate::generated::System;

/// Turn-, round- and activation-scoped stamps of cards and abilities that fit no rule area, among them the stamps
/// the played action cards leave (package B6): Morale Boost, Rout, Waylay, Maneuvering Jets and Shields Holding,
/// Intercept, Flank Speed, Hack Election, Distinguished Councilor and Bribery, Nav Suite, In The Silence Of Space.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CardTimers {
    pub agency_supply_used_turn: Opt32,
    pub fleet_logistics_used_turn: Opt32,
    /// Morale Boost: the combat round it applies to.
    pub combat_bonus_round: Opt32,
    /// Rout: the combat round.
    pub rout_round: Opt32,
    /// Waylay: the combat round.
    pub waylay_barrage_round: Opt32,
    /// Intercept: the combat round the retreat is barred in.
    pub retreat_barred_round: Opt32,
    /// Maneuvering Jets, Shields Holding: (round, hits left to cancel).
    pub cancel_hits_round: Option<(u32, u32)>,
    /// Flank Speed: the activation.
    pub move_bonus_activation: Opt32,
    /// Hack Election: the agenda.
    pub hack_votes_last_agenda: Opt32,
    /// Distinguished Councilor, Bribery: the extra votes of an agenda.
    pub extra_votes_agenda: Option<ExtraVotes>,
    /// Nav Suite: the activation.
    pub anomalies_ignored_activation: Opt32,
    /// In The Silence Of Space: the activation and the system.
    pub silence_activation: Opt32,
    pub silence_system: Option<System>,
}
