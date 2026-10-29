use crate::containers::Opt32;

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
    pub gravity_drive_used_activation: Opt32,
    pub spatial_conduit: Opt32,
}
