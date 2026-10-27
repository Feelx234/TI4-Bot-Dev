//! Decision types of the `production` area: production, unit placement, cost payment.
//!
//! Owner: package B3 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the payment question of a bill
//! (`Ask::Pay`). Everything an option's text and preview need is a number carried by the option, because
//! the renderer (`ti4-engine/src/render/production.rs`) sees no state. Never text; every type is `Copy`.

use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;

/// A currency of a bill or of a planet's face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Currency {
    Resources,
    Influence,
}

/// The payment question: one step of a bill of `kind` with `owed` still to pay, when the player could
/// spend `pool` of that currency and holds `goods` trade goods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PayAsk {
    pub kind: Currency,
    pub owed: i64,
    pub pool: i64,
    pub goods: i64,
}

/// Numbers an option shows (its worth, the pool after it): derived from the position, so they are not part
/// of the option's identity. Two options are equal when they name the same thing; `Shown` compares equal,
/// orders equal and hashes to nothing, so an answer parsed from an id equals the option that was offered.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shown {
    pub worth: i32,
    pub pool_after: i32,
}

impl PartialEq for Shown {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Shown {}
impl PartialOrd for Shown {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Shown {
    fn cmp(&self, _: &Self) -> core::cmp::Ordering {
        core::cmp::Ordering::Equal
    }
}
impl core::hash::Hash for Shown {
    fn hash<H: core::hash::Hasher>(&self, _: &mut H) {}
}

/// Where a produced unit may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spot {
    Space,
    Planet(Planet),
    /// A place in another system a module offers (`production_destinations`).
    Remote(System, Option<Planet>),
}

/// The placement question: where `made` bought copies of `unit` go, when there is more than one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceAsk {
    pub who: Seat,
    pub system: System,
    pub unit: Unit,
    pub made: u8,
    pub fleet_limit: i32,
}

/// The production question: what to build now. The window's own numbers (the string `ProductionWindow`
/// fields the options depend on); the rest an option shows is read from the position by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChooseAsk {
    pub who: Seat,
    pub system: System,
    /// Full PRODUCTION limit of this use and what is left of it.
    pub limit: i32,
    pub remaining: i32,
    /// Capacity ships opened for small units (Bellum Gloriosum) and the Cabal commander's exemptions.
    pub free_capacity: i32,
    pub small_exemptions: i32,
    /// Resources already paid in advance and the cost discount still available.
    pub credit: i32,
    pub discount_remaining: i32,
    /// The seat's fleet limit as `fleet::limit` works it out (pool, laws, abilities, the lifted-limit round).
    pub fleet_limit: i32,
    /// Harrugh Gefhara: every build of this use is free.
    pub free_this_use: bool,
    /// A Nomad commander waives flagship resources.
    pub flagship_free: bool,
    pub only_unit: Option<Unit>,
    pub max_unit_cost: Option<i32>,
    /// Units produced so far in this use, per unit base type (`UnitBaseType` order).
    pub produced: [u8; 11],
}

/// What the area asks. Parameters only; the option list is derived by `rules::production::pay_opts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Pay a step of a bill.
    Pay(PayAsk),
    /// Choose what to build.
    Choose(ChooseAsk),
    /// Choose where to put what was produced.
    Place(PlaceAsk),
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Exhaust a planet card for `worth` through its `source` face; the pool falls to `pool_after`.
    Exhaust { planet: Planet, source: Currency, shown: Shown },
    /// Spend a trade good (worth `shown.worth` against the bill).
    TradeGood { shown: Shown },
    /// Spend a commodity as a trade good (Keleres' agent).
    Commodity { shown: Shown },
    /// Build `made` copies of `unit`, paying for it (`build|unit|made`).
    Build { unit: Unit, made: u8 },
    /// Produce one `unit` by returning a captured one (`exchange|unit`).
    Exchange { unit: Unit },
    /// Place the produced unit at a spot (`place|spot`).
    Place { spot: Spot },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    Pay(Currency),
    Choose,
    Place,
}
