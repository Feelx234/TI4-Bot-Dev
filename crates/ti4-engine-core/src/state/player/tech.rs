use crate::containers::{Set, SortedMap};
use crate::generated::Technology;

/// All 102 technologies as a 16-byte bitset.
pub type TechSet = Set<Technology, 2>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerTech {
    pub owned: TechSet,
    pub exhausted: TechSet,
    /// `Player.assimilated_technologies`: the copied technology -> the technology it stands for.
    pub assimilated: SortedMap<Technology, Technology, 2>,
}
