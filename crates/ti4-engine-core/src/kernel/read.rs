//! `Rd`: the read bundle of the combat and invasion rules (packages B1 and B2 each had a copy; an L4 area
//! does not import another, so the one shared definition sits here in L3).

use crate::generated::{Unit, UnitBaseType};
use crate::kernel::seam::Q;
use crate::kernel::Kernel;
use crate::state::CoreState;
use crate::tables::Scope;

/// What a combat or invasion rule reads: the state and the kernel (content scope, hook table, seams).
#[derive(Clone, Copy)]
pub struct Rd<'a> {
    pub s: &'a CoreState,
    pub k: &'a Kernel,
    /// Law's Order: every law is blank this turn (`laws::blanked`; the caller computes it).
    pub blanked: bool,
    /// Invasion only: the leaders package's answer to `has_commander_ability(l1z1xcommander)` for the invader
    /// (it needs the leaders `Env`: Imperia, Alliance notes); `None`: read the leader slice and the grant mark only.
    pub l1z1x_commander: Option<bool>,
}

impl<'a> Rd<'a> {
    #[must_use]
    pub fn new(s: &'a CoreState, k: &'a Kernel) -> Self {
        Self { s, k, blanked: false, l1z1x_commander: None }
    }

    /// The same read with the invader's L1Z1X commander ability decided by the leaders package.
    #[must_use]
    pub fn with_l1z1x_commander(mut self, held: bool) -> Self {
        self.l1z1x_commander = Some(held);
        self
    }

    /// The seam reader over the same state and kernel.
    #[must_use]
    pub fn q(&self) -> Q<'a> {
        Q::new(self.s, self.k)
    }

    /// The same read with laws blank or not.
    #[must_use]
    pub fn with_blanked(mut self, blanked: bool) -> Self {
        self.blanked = blanked;
        self
    }

    /// The same read over another state (a played-out copy).
    #[must_use]
    pub fn over(self, s: &'a CoreState) -> Self {
        Self { s, ..self }
    }

    /// The source scope as the tables read it.
    #[must_use]
    pub fn scope(&self) -> Scope {
        Scope(u32::from(self.k.sources.0))
    }

    /// The unit's table row when the unit is in the game's scope (the string `catalogue(content,
    /// sources).get(id)`), else `None`.
    #[must_use]
    pub fn data(&self, unit: Unit) -> Option<&'static crate::generated::UnitData> {
        self.scope().unit(unit)
    }

    /// `UnitType::is_ship` (false for a unit out of scope).
    #[must_use]
    pub fn is_ship(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.ship)
    }

    /// `UnitType::is_fighter`.
    #[must_use]
    pub fn is_fighter(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.base_type == UnitBaseType::Fighter)
    }
}
