//! `StrategyState`: unclaimed cards, trade goods on cards, initiative.

use crate::containers::SortedMap;
use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::containers::SmallList;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StrategyState {
    pub unclaimed: SmallList<StrategyCard, 8>,
    pub goods: SortedMap<StrategyCard, i32, 12>,
    pub initiative: SortedMap<StrategyCard, i32, 12>,
    pub initiative_overrides: SortedMap<Seat, i32, 8>,
}
