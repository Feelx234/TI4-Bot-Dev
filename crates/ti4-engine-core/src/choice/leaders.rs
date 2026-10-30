//! Decision types of the `leaders` area: leaders, faction abilities, hero and agent plumbing.
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far, by the driver package D1 as the
//! worked example of a window (docs/ENGINE_CORE_DRIVER.md): the Rear Admiral Farran offer. The adapter
//! side is `ti4-engine/src/render/leaders.rs`.

use crate::generated::{Leader, Planet, Unit};
use crate::ids::Seat;

/// The unit a round agent (Evelyn De Louis, Viscount Unlenn) gives an extra combat die: its owner, where it
/// stands (`None`: in the fleet), its type, its ordinal in the owner's roll roster there and the two marks the
/// signature checks (the string `UnitTarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoundTarget {
    pub owner: Seat,
    pub planet: Option<Planet>,
    pub unit: Unit,
    pub index: u8,
    pub sustained: bool,
    pub galvanized: bool,
}

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
    /// Rin, the Masters' Legacy (Jol-Nar hero): replace this technology with another of its colour, or keep it.
    JolnarSwap { held: crate::generated::Technology },
    /// A round agent: "choose 1 unit in the active system: it rolls 1 additional die during this combat round".
    RoundAgentUnit { agent: Leader, copied: bool },
    /// Xxekir Grom (Thunder's Edge Xxcha hero): place another PDS or mech, or stop ("how many more?").
    XxchaHeroPlace,
    /// Xxekir Grom: which planet gets the unit.
    XxchaHeroPlanet,
    /// Gurno Aggero (Saar hero): which system next to a space dock loses the other players' infantry and fighters.
    SaarHero,
}

/// The kind of unit Xxekir Grom places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeroUnit {
    Pds,
    Mech,
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
    /// A system (the Helmsman's destination, Gurno Aggero's target).
    System(crate::generated::System),
    /// Jol-Nar hero: trade for this technology.
    Trade(crate::generated::Technology),
    /// This unit rolls the additional die.
    RoundAgentUnit(RoundTarget),
    /// Xxekir Grom: place 1 PDS / 1 mech.
    HeroPlace(HeroUnit),
    /// Xxekir Grom: stop placing.
    HeroStop,
    /// Xxekir Grom: this planet gets the unit, with the system the map shows it in (`Shown`, B9: equal always).
    HeroPlanet(crate::generated::Planet, super::agenda::Shown),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    SustainCommander,
    XxchaReadyPlanet,
    XxchaRemoveInfantry,
    HacanBranch,
    L1z1xDestination,
    JolnarSwap,
    RoundAgentUnit,
    XxchaHeroPlace,
    XxchaHeroPlanet,
    SaarHero,
}
