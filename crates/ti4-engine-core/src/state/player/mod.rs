//! `Player`, split into per-owner sub-structs so packages own disjoint parts.

mod card_timers;
mod cards;
mod combat_timers;
mod econ;
mod leaders;
mod misc;
mod production_timers;
mod round_marks;
mod tech;
mod timers;

pub use card_timers::CardTimers;
pub use cards::{FragmentCounts, Plot, PlotCard, PlayerCards, ReactionMode};
pub use combat_timers::CombatTimers;
pub use econ::PlayerEcon;
pub use leaders::{LeaderStatus, PlayerLeaders};
pub use misc::{Feat, PlayerMisc};
pub use production_timers::ProductionTimers;
pub use round_marks::{RoundKind, RoundMarks};
pub use tech::{PlayerTech, TechSet};
pub use timers::{ExtraVotes, Timers};

use crate::containers::{Set, SmallList};
use crate::generated::{Faction, Planet, StrategyCard, System};
use crate::ids::Seat;

/// One player. `size_of` target 600 B (docs/ENGINE_CORE_STATE.md 1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// The player's owner seat (`Player.id`).
    pub seat: Seat,
    pub faction: Faction,
    pub home_system: Option<System>,
    pub home_planets: SmallList<Planet, 4>,
    pub strat: SmallList<StrategyCard, 3>,
    pub strat_exhausted: Set<StrategyCard, 1>,
    pub econ: PlayerEcon,
    pub tech: PlayerTech,
    pub cards: PlayerCards,
    pub leaders: PlayerLeaders,
    pub timers: Timers,
    pub combat_timers: CombatTimers,
    pub production_timers: ProductionTimers,
    pub card_timers: CardTimers,
    pub round_marks: RoundMarks,
    pub misc: PlayerMisc,
}

impl Default for Player {
    fn default() -> Self {
        Self::new(Seat(0), Faction::from_index(0).expect("a faction exists"))
    }
}

impl Player {
    #[must_use]
    pub fn new(seat: Seat, faction: Faction) -> Self {
        Self {
            seat,
            faction,
            home_system: None,
            home_planets: SmallList::new(),
            strat: SmallList::new(),
            strat_exhausted: Set::new(),
            econ: PlayerEcon::default(),
            tech: PlayerTech::default(),
            cards: PlayerCards::default(),
            leaders: PlayerLeaders::default(),
            timers: Timers::default(),
            combat_timers: CombatTimers::default(),
            production_timers: ProductionTimers::default(),
            card_timers: CardTimers::default(),
            round_marks: RoundMarks::default(),
            misc: PlayerMisc::default(),
        }
    }
}
