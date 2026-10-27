use crate::containers::SortedMap;
use crate::generated::{Planet, System};

/// Counters of the current production use (`fighters_produced_this_use` .. `gravleash_move_values`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductionCounters {
    pub fighters_produced_this_use: i32,
    pub nonfighter_ships_produced_this_use: i32,
    pub units_produced_this_use: i32,
    pub discount_remaining: i32,
    pub value_swapped_planet: Option<Planet>,
    pub gravleash_move_values: SortedMap<System, i32, 4>,
}
