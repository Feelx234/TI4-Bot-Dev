//! Why a unit was destroyed: `GameState::pending_destructions` and the cause of a staged ground
//! destruction.

use crate::generated::{Agenda, Leader};

/// The action card effect behind an `action_card:` cause (the effect's name, which is not always
/// the card's id: `unstable_planet` is the card `unstable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionCardCause {
    Courageous,
    DirectHit,
    Plague,
    ReflectiveShielding,
    UnstablePlanet,
}

/// The ability behind a `faction_ability:` cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactionAbilityCause {
    Devotion,
}

/// The ability behind a `unit_ability:` cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitAbilityCause {
    AntiFighterBarrage,
    SpaceCannon,
    YinFlagship,
}

/// What destroyed a unit. The causes come from three families of writers: fixed words of the
/// combat and card code, `<kind>:<id>` pairs built from the name of the card, leader or agenda that did it, and the
/// older `<kind>_<id>` / bare-id forms of the ground-destruction stager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DestroyReason {
    Bombardment,
    /// `breakthrough:nova_seed`.
    BreakthroughNovaSeed,
    GroundCombat,
    LastDispatch,
    /// The bare `nova_seed` of the ground stager.
    NovaSeed,
    PlotSeethe,
    RetreatCapacity,
    SpaceCombat,
    Sundered,
    /// `technology:impulse_core`.
    TechnologyImpulseCore,
    /// `technology_parasite_obs`.
    TechnologyParasiteObs,
    /// `action_card:<id>`.
    ActionCard(ActionCardCause),
    /// `agenda:<alias>`.
    Agenda(Agenda),
    /// `faction_ability:<name>`.
    FactionAbility(FactionAbilityCause),
    /// `leader:<id>`.
    Leader(Leader),
    /// `leader_<id>`.
    LeaderUnderscore(Leader),
    /// A leader's bare id (`nekrohero`).
    LeaderBare(Leader),
    /// `unit_ability:<name>`.
    UnitAbility(UnitAbilityCause),
}
