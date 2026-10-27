use crate::containers::{Set, SeatSet, SmallList, SortedMap};
use crate::generated::{ActionCard, Breakthrough, Explore, ExploreType, Planet, Relic, SecretObjective};

/// `ti4_model::state::ReactionMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ReactionMode {
    #[default]
    Always,
    Never,
    OwnTurn,
}

/// The five plot cards, in text order of their ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlotCard {
    Assail,
    Enervate,
    Extract,
    Seethe,
    Siphon,
}

/// `ti4_model::plots::Plot`: a plot in a player's hand (card hidden when `card` is `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Plot {
    pub card: Option<PlotCard>,
    pub tokens: SeatSet,
    pub faceup: bool,
}

/// `Player.relic_fragments`: counts by exploration trait (cultural, frontier ("unknown"), hazardous,
/// industrial: indexed by [`ExploreType`], the text order of the keys). `present` records which keys
/// the map holds, because a zero count is a present key in the string state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FragmentCounts {
    pub counts: [i16; 4],
    pub present: u8,
}

impl FragmentCounts {
    #[must_use]
    pub fn get(&self, kind: ExploreType) -> Option<i16> {
        (self.present & (1 << kind as u8) != 0).then(|| self.counts[kind as usize])
    }

    pub fn set(&mut self, kind: ExploreType, count: i16) {
        self.present |= 1 << kind as u8;
        self.counts[kind as usize] = count;
    }

    pub fn remove(&mut self, kind: ExploreType) {
        self.present &= !(1 << kind as u8);
        self.counts[kind as usize] = 0;
    }
}

/// Cards in a player's hand and play area. Hand order is observable (option order, JSON).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerCards {
    pub hand: SmallList<ActionCard, 10>,
    pub reaction_modes: SortedMap<ActionCard, ReactionMode, 4>,
    pub secrets: SmallList<SecretObjective, 4>,
    pub relics: SmallList<Relic, 4>,
    pub relics_exhausted: Set<Relic, 1>,
    pub legendary_exhausted: SortedMap<Planet, (), 2>,
    pub fragments: FragmentCounts,
    pub explore: SmallList<Explore, 4>,
    pub plots: SmallList<Plot, 3>,
    pub plot_objectives: Set<SecretObjective, 1>,
    pub breakthrough: Option<Breakthrough>,
}
