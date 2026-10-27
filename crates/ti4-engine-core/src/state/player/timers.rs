use crate::containers::Opt32;
use crate::generated::System;

/// `Player.extra_votes_agenda`: the agenda sequence and the votes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraVotes {
    pub agenda: u32,
    pub votes: i64,
}

/// Activation- and agenda-scoped stamps (movement and voting side). `Opt32` = `Option<u32>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Timers {
    pub conduit_activation: Opt32,
    pub move_bonus_activation: Opt32,
    pub gravity_drive_used_activation: Opt32,
    pub anomalies_ignored_activation: Opt32,
    pub silence_activation: Opt32,
    pub silence_system: Option<System>,
    pub spatial_conduit: Opt32,
    pub hack_votes_last_agenda: Opt32,
    pub extra_votes_agenda: Option<ExtraVotes>,
}
