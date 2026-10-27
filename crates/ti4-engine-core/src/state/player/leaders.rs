use crate::containers::SortedMap;
use crate::generated::Leader;

/// `ti4_model::state::LeaderStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LeaderStatus {
    Locked,
    Readied,
    Exhausted,
    Unlocked,
    Purged,
}

/// `Player.leaders`: a leader -> status map in leader (text) order. A sorted small map rather than
/// four slots, because the string map can hold any leader the faction or a card gave the player.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerLeaders {
    pub status: SortedMap<Leader, LeaderStatus, 5>,
}
