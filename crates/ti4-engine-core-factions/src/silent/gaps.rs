//! The LOUD gaps of the live path: string abilities the core table has no rule for yet, in scope, each a row here. A native game registers a row for every
//! seat (`register_live`), and when the seat holds the card and its event is announced the condition answers `Unknown::AbilityGap(slot)`: the live step stops
//! with a counted bail instead of playing on without the ability (the dual does not register them: the string runs next to the core there, and
//! `Recording::unported` already bails a window that finds one eligible).
//!
//! The predicates are coarse on purpose (the seat plays the faction, holds the note or the technology): a gap that fires too often is a reason to port the
//! ability, a gap that never fires is a silent wrong game. `ti4-engine`'s `ability_audit` pins the list against what the string registers.
//! A row's index is its slot (`FIRST_SLOT + index`) and the position of its name in the engine's table.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, PromissoryNote, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{no_effect, AbilityTable, CondCx, Slot, Spec, Unknown};
use ti4_engine_core::state::EventKind;

/// The slot of the first row.
pub const FIRST_SLOT: u16 = 41_000;

/// Suffi An (the Mentak agent) and Ssruu's copy of it: the rows Pillage's take stops at.
pub const SUFFI_AN: usize = 7;
pub const SUFFI_AN_COPY: usize = 44;

/// One gap: the registered ability's kind, event and relation, and who could use it.
pub struct Gap {
    pub kind: AbilityKind,
    pub event: EventKind,
    pub relation: Relation,
    pub holds: fn(&CondCx<'_>) -> bool,
}

fn faction(c: &CondCx<'_>, f: Faction) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.faction == f)
}

/// The seat plays Yssaril (Ssruu) and a seat of `lender` is at the table.
fn copies(c: &CondCx<'_>, lender: Faction) -> bool {
    faction(c, Faction::TheYssarilTribes) && c.state.players().iter().any(|p| p.faction == lender)
}

fn tech(c: &CondCx<'_>, t: Technology) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.tech.owned.contains(t))
}

fn note(c: &CondCx<'_>, n: PromissoryNote) -> bool {
    let me: Seat = c.def.owner;
    c.state.cards().promissory.iter().any(|(key, holder)| key.note == n && *holder == me)
}

/// The rows, in the order of the engine's name table (`ability_audit::GAP_NAMES`).
pub const GAPS: &[Gap] = &[
    // -- abilities
    Gap { kind: AbilityKind::Ability, event: EventKind::MovementFinished, relation: Relation::After, holds: |c| faction(c, Faction::TheNaaluCollective) }, // foresight
    Gap { kind: AbilityKind::Ability, event: EventKind::PropagationResearch, relation: Relation::After, holds: |c| faction(c, Faction::TheNekroVirus) }, // propagation
    Gap { kind: AbilityKind::Ability, event: EventKind::ActionCompleted, relation: Relation::After, holds: |c| faction(c, Faction::TheWinnu) }, // reclamation
    Gap { kind: AbilityKind::Ability, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| faction(c, Faction::TheWinnu) }, // reclamation_clear
    Gap { kind: AbilityKind::Ability, event: EventKind::PlanetControlGained, relation: Relation::After, holds: |c| faction(c, Faction::TheWinnu) }, // reclamation_mark
    // -- leaders
    Gap { kind: AbilityKind::Leader, event: EventKind::UnitAbilityRolled, relation: Relation::When, holds: |c| faction(c, Faction::TheArgentFlight) }, // argentcommander
    Gap { kind: AbilityKind::Leader, event: EventKind::CommoditiesReplenished, relation: Relation::After, holds: |c| faction(c, Faction::TheVuilraithCabal) }, // cabalagent
    Gap { kind: AbilityKind::Leader, event: EventKind::PillageUsed, relation: Relation::After, holds: |c| faction(c, Faction::TheMentakCoalition) }, // mentakagent
    Gap { kind: AbilityKind::Leader, event: EventKind::SpaceCombatWon, relation: Relation::After, holds: |c| faction(c, Faction::TheMentakCoalition) }, // mentakcommander
    Gap { kind: AbilityKind::Leader, event: EventKind::StrategyTokenSpent, relation: Relation::After, holds: |c| faction(c, Faction::TheEmbersOfMuaat) }, // muaatcommander
    Gap { kind: AbilityKind::Leader, event: EventKind::UnitsProduced, relation: Relation::After, holds: |c| faction(c, Faction::TheEmbersOfMuaat) }, // muaatcommander_unlock
    Gap { kind: AbilityKind::Leader, event: EventKind::CommandTokenPlaced, relation: Relation::After, holds: |c| faction(c, Faction::TheNaaluCollective) }, // naaluagent-te
    Gap { kind: AbilityKind::Leader, event: EventKind::StatusPhaseEnded, relation: Relation::After, holds: |c| faction(c, Faction::TheNaaluCollective) }, // naaluhero
    Gap { kind: AbilityKind::Leader, event: EventKind::TurnPassed, relation: Relation::After, holds: |c| faction(c, Faction::TheNaazRokhaAlliance) }, // naazagent
    Gap { kind: AbilityKind::Leader, event: EventKind::PlanetControlGained, relation: Relation::After, holds: |c| faction(c, Faction::TheNaazRokhaAlliance) }, // naazcommander
    Gap { kind: AbilityKind::Leader, event: EventKind::TechnologyGained, relation: Relation::After, holds: |c| faction(c, Faction::TheNekroVirus) }, // nekrocommander
    Gap { kind: AbilityKind::Leader, event: EventKind::StrategicActionBegan, relation: Relation::After, holds: |c| faction(c, Faction::SardakkNorr) }, // sardakkagent_strategic
    Gap { kind: AbilityKind::Leader, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| faction(c, Faction::SardakkNorr) }, // sardakkagent_turn
    Gap { kind: AbilityKind::Leader, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| faction(c, Faction::SardakkNorr) }, // sardakkhero_turn
    Gap { kind: AbilityKind::Leader, event: EventKind::GroundCombatStarted, relation: Relation::After, holds: |c| faction(c, Faction::TheFederationOfSol) }, // solcommander
    Gap { kind: AbilityKind::Leader, event: EventKind::ProductionUsed, relation: Relation::When, holds: |c| faction(c, Faction::TheTitansOfUl) }, // titanscommander
    Gap { kind: AbilityKind::Leader, event: EventKind::ProductionUsed, relation: Relation::After, holds: |c| faction(c, Faction::TheWinnu) }, // winnuagent
    // -- promissory notes (held by any seat)
    Gap { kind: AbilityKind::Promissory, event: EventKind::UnitAbilityRolled, relation: Relation::When, holds: |c| note(c, PromissoryNote::StrikeWingAmbuscade) }, // ambuscade
    Gap { kind: AbilityKind::Promissory, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| note(c, PromissoryNote::CreussIff) }, // iff
    Gap { kind: AbilityKind::Promissory, event: EventKind::UnitsCommitted, relation: Relation::After, holds: |c| note(c, PromissoryNote::RaghsCall) }, // ragh
    Gap { kind: AbilityKind::Promissory, event: EventKind::StrategyPhaseBegan, relation: Relation::After, holds: |c| note(c, PromissoryNote::ScepterOfDominion) }, // scepter
    Gap { kind: AbilityKind::Promissory, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| note(c, PromissoryNote::SpyNet) }, // spynet
    Gap { kind: AbilityKind::Promissory, event: EventKind::ShipMoved, relation: Relation::After, holds: |c| note(c, PromissoryNote::Stymie) }, // stymie
    Gap { kind: AbilityKind::Promissory, event: EventKind::GroundCombatStarted, relation: Relation::After, holds: |c| note(c, PromissoryNote::TekklarLegion) }, // tekklar
    Gap { kind: AbilityKind::Promissory, event: EventKind::GroundCombatEnded, relation: Relation::After, holds: |c| note(c, PromissoryNote::TekklarLegion) }, // tekklar_end
    Gap { kind: AbilityKind::Promissory, event: EventKind::TurnBegan, relation: Relation::After, holds: |c| note(c, PromissoryNote::TekklarLegion) }, // tekklar_turn
    // -- technologies
    Gap { kind: AbilityKind::Technology, event: EventKind::ActionCompleted, relation: Relation::After, holds: |c| tech(c, Technology::Aetherstream) }, // asn_clear
    Gap { kind: AbilityKind::Technology, event: EventKind::StatusPhaseEnded, relation: Relation::After, holds: |c| tech(c, Technology::Bioplasmosis) }, // bio
    Gap { kind: AbilityKind::Technology, event: EventKind::SpaceCombatRoundEnded, relation: Relation::After, holds: |c| tech(c, Technology::ExotriremeII) }, // exo2
    Gap { kind: AbilityKind::Technology, event: EventKind::ProductionUsed, relation: Relation::After, holds: |c| tech(c, Technology::HegemonicTradePolicy) }, // htp
    Gap { kind: AbilityKind::Technology, event: EventKind::GroundCombatStarted, relation: Relation::After, holds: |c| tech(c, Technology::MagenDefenseGridOmegaOmega) }, // md_hit
    Gap { kind: AbilityKind::Technology, event: EventKind::ShipDestroyed, relation: Relation::After, holds: |c| tech(c, Technology::NULLREFERENCE) }, // nekroc4y
    Gap { kind: AbilityKind::Technology, event: EventKind::UnitAbilityRolled, relation: Relation::After, holds: |c| tech(c, Technology::StrikeWingAlphaII) }, // swa2
    Gap { kind: AbilityKind::Technology, event: EventKind::MovementFinished, relation: Relation::After, holds: |c| tech(c, Technology::Voidwatch) }, // vw
    Gap { kind: AbilityKind::Technology, event: EventKind::UnitsProduced, relation: Relation::After, holds: |c| tech(c, Technology::YinSpinnerOmega) }, // yso
    // -- units
    Gap { kind: AbilityKind::Unit, event: EventKind::RelicGained, relation: Relation::After, holds: |c| faction(c, Faction::TheNaaluCollective) }, // naalu_mech_te
    Gap { kind: AbilityKind::Unit, event: EventKind::SpaceCombatStarted, relation: Relation::After, holds: |c| faction(c, Faction::TheNekroVirus) }, // nekro_flagship
    Gap { kind: AbilityKind::Unit, event: EventKind::ActionCompleted, relation: Relation::After, holds: |c| faction(c, Faction::TheWinnu) }, // winnu_mech
    // -- ported by no lane yet
    Gap { kind: AbilityKind::Unit, event: EventKind::PlanetControlGained, relation: Relation::After, holds: |c| faction(c, Faction::TheClanOfSaar) }, // saar_mech
    // -- the copies Ssruu makes of an agent of a seated faction
    Gap { kind: AbilityKind::Leader, event: EventKind::PillageUsed, relation: Relation::After, holds: |c| copies(c, Faction::TheMentakCoalition) }, // yssarilagent: mentakagent
    Gap { kind: AbilityKind::Leader, event: EventKind::TurnPassed, relation: Relation::After, holds: |c| copies(c, Faction::TheNaazRokhaAlliance) }, // yssarilagent: naazagent
    Gap { kind: AbilityKind::Leader, event: EventKind::ProductionUsed, relation: Relation::After, holds: |c| copies(c, Faction::TheWinnu) }, // yssarilagent: winnuagent
    // -- the activation abilities lane M6c lists as gaps (its audit); delete the row when the lane ports the rule
];

/// The condition of every row: the seat could use the ability, and the core cannot say whether it is eligible now.
fn gap_condition(c: &CondCx<'_>) -> bool {
    let Some(index) = usize::try_from(c.def.arg).ok().and_then(|arg| arg.checked_sub(usize::from(FIRST_SLOT))) else { return false };
    GAPS.get(index).is_some_and(|gap| (gap.holds)(c)) && c.unknown(Unknown::AbilityGap(Slot(FIRST_SLOT + u16::try_from(index).unwrap_or(0))))
}

/// The slot of row `index`.
#[must_use]
pub const fn slot(index: usize) -> Slot {
    Slot(FIRST_SLOT + index as u16)
}

/// Register every row for every seat (the live path's table only).
pub fn register_live(t: &mut AbilityTable) {
    for (index, gap) in GAPS.iter().enumerate() {
        t.add(Spec::new(slot(index), gap.kind, gap.event, gap.relation, no_effect).when(gap_condition));
    }
}
