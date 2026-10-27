use crate::containers::Opt32;

/// Round- and use-scoped production effects.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductionTimers {
    pub fleet_supply_unlimited_until: Opt32,
    pub free_production_use: Opt32,
}
