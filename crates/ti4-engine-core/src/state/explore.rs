//! `ExploreState`: frontier, ingress and breach tokens, expedition slices, wormhole tokens, the log.
//! (Planet attachments live in `PlanetCell.attach`.)

use crate::containers::{Log, Set, SortedMap};
use crate::generated::{Explore, ExploreType, Planet, System, Wormhole};
use crate::ids::Seat;

use super::Code;

/// The Thunder's Edge expedition slices, in text order of their keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expedition {
    ActionCards,
    Influence,
    Resources,
    Secret,
    TechPlanet,
    TradeGoods,
}

/// `ExplorationRecord`. The open string sets (`outcome`, `result`, gain keys, placed tokens) are
/// adapter-interned [`Code`]s until X1 pins their values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreRec {
    pub round: u32,
    pub player: Seat,
    pub deck: ExploreType,
    pub card: Explore,
    pub planet: Option<Planet>,
    pub outcome: Code,
    pub system: Option<System>,
    pub result: Option<Code>,
    pub gains: SortedMap<Code, i32, 4>,
    pub placed: crate::containers::SmallList<Code, 2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExploreState {
    pub frontier: Set<System, 4>,
    pub ingress: Set<System, 4>,
    pub breach: Set<System, 4>,
    pub expedition_slices: SortedMap<Expedition, Seat, 6>,
    /// `GameState.wormhole_tokens` (Creuss tokens: alpha and beta).
    pub wormhole_tokens: SortedMap<Wormhole, System, 5>,
    pub log: Log<ExploreRec>,
}
