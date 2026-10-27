//! Decision types of the `invasion` area: invasion, commit ground forces, landing.
//!
//! Owner: package B2 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: committing ground forces to planets
//! (LRR 49.2). The adapter side is `ti4-engine/src/render/invasion.rs`.

use crate::generated::{Planet, System, Unit};

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
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Land the unit at `index` of the caller's list on `planet`.
    Commit { index: u16, planet: Planet, unit: Unit, damaged: bool },
    /// "commit no more ground forces".
    DoneCommitting,
    /// Destroy the ground force at `index` of the candidates (the first of its type and damage).
    DestroyGround { index: u16, unit: Unit, damaged: bool, staging: bool },
    /// Leave the custodians token ("no").
    KeepCustodians,
    /// Remove the custodians token for a victory point ("yes").
    RemoveCustodians,
    /// Roll one round of the ground combat on the planet.
    FightRound { planet: Planet },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    CommitGroundForces,
    AssignGroundCasualty,
    RemoveCustodians,
    FightGroundCombatRound,
}
