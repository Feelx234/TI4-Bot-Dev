//! `Transient`: the `last_*` slots, combat and invasion scratch, production counters, movement scratch and the
//! turn-flow flags. Split by area (combat B1, invasion B2, production B3) so owners edit disjoint files.

mod combat;
mod invasion;
mod misc;
mod movement;
mod production;

pub use combat::{CombatDie, CombatPresentation, CombatScratch, CombatStage, DestroyReason, PendingDestruction, RerollEntry, RerollKind, RerollSet};
pub use invasion::{ActiveInvasion, InvasionDie, InvasionKind, InvasionPhase, InvasionScratch, InvasionStep};
pub use misc::{MiscScratch, StartingTechPick, StartingTechSource, TechList, TransientFlags};
pub use movement::MovementScratch;
pub use production::ProductionCounters;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transient {
    pub combat: CombatScratch,
    pub invasion: InvasionScratch,
    pub production: ProductionCounters,
    pub movement: MovementScratch,
    pub misc: MiscScratch,
}
