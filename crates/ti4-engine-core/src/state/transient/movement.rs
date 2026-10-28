use crate::containers::SortedMap;
use crate::generated::System;

/// Scratch of the tactical action's movement step, split out of the production counters (it was never a
/// production fact): Letnev's Gravleash Maneuvers keeps, for each origin system, the highest move value among
/// the ships already selected to move from it, so that later slower hulls inherit it
/// (`gravleash_move_values`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MovementScratch {
    pub gravleash_move_values: SortedMap<System, i32, 4>,
}

#[cfg(test)]
mod tests {
    use super::super::{ProductionCounters, Transient};

    #[test]
    fn the_gravleash_scratch_is_not_a_production_counter() {
        let mut t = Transient::default();
        t.movement.gravleash_move_values.insert(crate::ids::system_at(30), 2);
        t.production = ProductionCounters { units_produced_this_use: 3, ..ProductionCounters::default() };
        // Ending a production use resets its counters and leaves the movement scratch alone.
        t.production = ProductionCounters::default();
        assert_eq!(t.movement.gravleash_move_values.get(&crate::ids::system_at(30)), Some(&2));
    }
}
