//! The typed sites the seams are asked about. `Copy`, no text: seats, generated ids and counts.

use super::data::{ModifierRow, RowKind};
use super::Q;
use crate::containers::{SeatSet, SmallList};
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;

pub use crate::kernel::hooks::{CombatCtx, CombatUnit, Spend};

/// What a die is rolled for (`RollKind` of the disentangle doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RollKind {
    Combat,
    SpaceCannon,
    Barrage,
    Bombardment,
    Ground,
}

/// A site that rows ([`ModifierRow`]) can be evaluated at: how many times the row applies, 0 for never.
pub trait Site {
    /// The multiplicity of `row` here (0: does not apply). The default: no row applies.
    fn row_times(&self, _q: Q<'_>, _row: &ModifierRow) -> i64 {
        0
    }
}

/// One roll about to be made: a unit's combat roll, a space cannon shot, a barrage, a bombardment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollSite {
    pub unit: CombatUnit,
    pub kind: RollKind,
    pub round: u32,
    /// The unit's index among the owner's units of its type in the system (`borrowed_round_agents::extra_die_for`).
    pub unit_index: u16,
    /// The seats this unit fights (the opponents of the combat or the target of the shot).
    pub foes: SeatSet,
}

impl RollSite {
    /// A site for tests and the empty-table probes.
    #[must_use]
    pub const fn probe() -> Self {
        Self { unit: CombatUnit::probe(), kind: RollKind::Combat, round: 1, unit_index: 0, foes: SeatSet::EMPTY }
    }

    fn is_fighter(&self) -> bool {
        self.unit.unit.data().base_type == UnitBaseType::Fighter
    }
}

impl Site for RollSite {
    fn row_times(&self, q: Q<'_>, row: &ModifierRow) -> i64 {
        if !row.rolls.has(self.kind) || (row.fighters_only && !self.is_fighter()) {
            return 0;
        }
        row.times(q, self.unit.owner, self.foes, self.unit.system, self.round)
    }
}

/// A planet's spend value: whose units read it, which planet, resources or influence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpendSite {
    pub who: Seat,
    pub planet: Planet,
    pub spend: Spend,
}
impl Site for SpendSite {}

/// A production: who, in which system, on which planet (`None`: the space area).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProdSite {
    pub who: Seat,
    pub system: System,
    pub planet: Option<Planet>,
}
impl Site for ProdSite {}

/// `cannot_produce(who, unit, producer)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProduceSite {
    pub who: Seat,
    pub unit: UnitBaseType,
    pub producer: UnitBaseType,
}
impl Site for ProduceSite {}

/// Where a piece's abilities may be switched off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SilenceCtx {
    Space,
    Ground,
    Production,
}

/// `silenced(piece, ctx)`: whose unit, where, which type, in what context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilenceSite {
    pub owner: Seat,
    pub system: System,
    pub unit: Option<Unit>,
    pub ctx: SilenceCtx,
}
impl Site for SilenceSite {}

/// `space_cannon_barred(shooter, target, system)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CannonSite {
    pub shooter: Seat,
    pub target: Seat,
    pub system: System,
}
impl Site for CannonSite {}

/// A seat acting in a system (Alastor participants, deploy planets, borrowed stats, commit blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatSystemSite {
    pub who: Seat,
    pub system: System,
}
impl Site for SeatSystemSite {}

impl Site for CombatUnit {}

/// Whether a row kind feeds a number stack (the others are read through `rows` helpers).
#[must_use]
pub const fn feeds_hit_shift(kind: &RowKind) -> bool {
    matches!(kind, RowKind::RollShift(_))
}

/// `production_destinations(who, system, base)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestSite {
    pub who: Seat,
    pub system: System,
    pub base: UnitBaseType,
}
impl Site for DestSite {}

/// `production_unit_exchange(who, system, unit)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExchangeSite {
    pub who: Seat,
    pub system: System,
    pub unit: Unit,
}
impl Site for ExchangeSite {}

/// `crimson_deploy_planets(invader, system, landable)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploySite {
    pub who: Seat,
    pub system: System,
    pub landable: SmallList<Planet, 4>,
}
impl Site for DeploySite {}

/// `keleres_units_commit_blocked(invader, system, planet)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitSite {
    pub who: Seat,
    pub system: System,
    pub planet: Planet,
}
impl Site for CommitSite {}
