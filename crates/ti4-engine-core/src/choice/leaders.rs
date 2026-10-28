//! Decision types of the `leaders` area: leaders, faction abilities, hero and agent plumbing.
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far, by the driver package D1 as the
//! worked example of a window (docs/ENGINE_CORE_DRIVER.md): the Rear Admiral Farran offer. The adapter
//! side is `ti4-engine/src/render/leaders.rs`.

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Rear Admiral Farran (Letnev commander): "After 1 of your units uses SUSTAIN DAMAGE: you may gain 1
    /// trade good". The actor is the commander's holder.
    SustainCommander,
    /// Ggrocuto Rinn (Xxcha agent), more than one exhausted planet: "ready which planet".
    XxchaPlanet,
    /// Ggrocuto Rinn: "remove 1 infantry from that planet?" (the readied planet is next to a system of the holder).
    XxchaInfantry,
    /// Carth of Golden Sands (Hacan agent): gain 2 commodities, or replenish another player.
    HacanBranch,
    /// The Helmsman (L1Z1X hero): which system to gather the flagship and dreadnoughts in.
    L1z1xDestination,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Gain the trade good.
    GainTradeGood,
    /// A planet to ready, with the system the map shows it in (the `system` payload of the option). `Shown`
    /// (B9) compares equal always, so an answer parsed from an id equals the option that was offered.
    Planet(crate::generated::Planet, super::agenda::Shown),
    /// Remove the infantry.
    Remove,
    /// Keep it.
    Keep,
    /// Hacan agent: gain 2 commodities.
    GainCommodities,
    /// Hacan agent: replenish this seat's commodities.
    Replenish(crate::ids::Seat),
    /// A system (the Helmsman's destination).
    System(crate::generated::System),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    SustainCommander,
    XxchaReadyPlanet,
    XxchaRemoveInfantry,
    HacanBranch,
    L1z1xDestination,
}
