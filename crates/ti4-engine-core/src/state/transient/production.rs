use crate::generated::Planet;

/// Counters of the current production use (`fighters_produced_this_use` .. `production_value_swapped_planet`); the movement scratch of the old struct lives in `MovementScratch`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductionCounters {
    pub fighters_produced_this_use: i32,
    pub nonfighter_ships_produced_this_use: i32,
    pub units_produced_this_use: i32,
    pub discount_remaining: i32,
    pub value_swapped_planet: Option<Planet>,
}
