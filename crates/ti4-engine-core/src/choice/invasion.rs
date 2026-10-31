//! Decision types of the `invasion` area: invasion, commit ground forces, landing.
//!
//! Owner: package B2 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: committing ground forces to planets
//! (LRR 49.2). The adapter side is `ti4-engine/src/render/invasion.rs`.

use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "commit ground forces in {system}": one option per distinguishable landing, then the end. The list of
    /// ground forces and the planets are the caller's (the window reads them from the board and the faction
    /// hooks); the options are derived from them by `rules::invasion::commit_opts`.
    CommitGround { system: System },
    /// "assign a hit on {planet}": the owner of the ground forces there picks which one is destroyed
    /// (`staging`: the options say whether the unit is damaged). The candidates are read from the board.
    AssignGroundHit { system: System, planet: Planet, staging: bool },
    /// "spend {cost} influence to remove the custodians token" (27.2); with `cost == 0` "remove the custodians
    /// token (no influence required)". Asked of the invader at the head of the commit step.
    RemoveCustodians { system: System, cost: u8 },
    /// "fight a round on {planet}": the single option that rolls one round of the ground combat (42).
    FightRound { system: System, planet: Planet },
    /// "start another ground combat on {planet}": the winner of a combat on a coexisting planet may fight the
    /// next coexister `next` (coexistence 12) or leave them coexisting.
    StartNextCombat { system: System, planet: Planet, next: Seat },
    /// "whose units on {planet} take the bombardment's next hits ({hits} hits)": the invader picks, per
    /// bombarding unit, the coexisting player whose ground forces take them (coexistence 7, 7.1).
    BombardTarget { system: System, planet: Planet, hits: u16 },
    /// Titans' Slumberstate Computing: "coexist on {planet} instead of fighting" where Coalescence forces a ground combat and the
    /// invader commits no other units; `holder` controls the planet.
    CoexistInstead { system: System, planet: Planet, holder: Seat },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Land the unit at `index` of the caller's list on `planet`; `from` is the planet it stands on when it is not
    /// in the space area (a faction's commit candidate).
    Commit { index: u16, planet: Planet, unit: Unit, damaged: bool, from: Option<(System, Planet)> },
    /// "commit no more ground forces".
    DoneCommitting,
    /// The Crimson Revenant's deploy: commit a mech from reinforcements to `planet`, with no unit of the
    /// invader in the system required.
    Deploy { planet: Planet },
    /// Destroy the ground force at `index` of the candidates (the first of its type and damage).
    DestroyGround { index: u16, unit: Unit, damaged: bool, staging: bool },
    /// Leave the custodians token ("no").
    KeepCustodians,
    /// Remove the custodians token for a victory point ("yes").
    RemoveCustodians,
    /// Roll one round of the ground combat on the planet.
    FightRound { planet: Planet },
    /// Start a ground combat against `next`.
    FightNext { next: Seat },
    /// Leave `next` coexisting (and end the chain).
    LeaveCoexisting { next: Seat },
    /// The units of `seat` take the hits.
    BombardTarget { seat: Seat },
    /// Slumberstate Computing: fight for the planet (the answer `fight`).
    CoexistFight,
    /// Slumberstate Computing: coexist on the planet instead (the answer `coexist`).
    CoexistInstead,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    CommitGroundForces,
    AssignGroundCasualty,
    RemoveCustodians,
    FightGroundCombatRound,
    StartNextGroundCombat,
    BombardmentTarget,
    CoalescenceCoexist,
}
