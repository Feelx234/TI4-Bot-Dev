use crate::containers::Opt32;

/// Turn- and round-scoped effects of cards and abilities that fit no rule area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CardTimers {
    pub agency_supply_used_turn: Opt32,
    pub fleet_logistics_used_turn: Opt32,
}
