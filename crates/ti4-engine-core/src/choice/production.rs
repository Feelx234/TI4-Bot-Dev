//! Decision types of the `production` area: production, unit placement, cost payment.
//!
//! Owner: package B3 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the payment question of a bill
//! (`Ask::Pay`). Everything an option's text and preview need is a number carried by the option, because
//! the renderer (`ti4-engine/src/render/production.rs`) sees no state. Never text; every type is `Copy`.

use crate::generated::Planet;

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
    pub worth: i64,
    pub pool_after: i64,
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

/// What the area asks. Parameters only; the option list is derived by `rules::production::pay_opts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Pay a step of a bill.
    Pay(PayAsk),
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
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    Pay(Currency),
}
