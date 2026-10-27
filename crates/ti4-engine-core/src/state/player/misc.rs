use crate::containers::SmallList;
use crate::generated::Unit;
use crate::ids::Seat;

/// `ti4_model::state::Feat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feat {
    DestroyedACapitalShip,
    BombardedOutTheLastGroundForces,
    SpaceCannonTookTheLastNonFighters,
    BarrageTookTheLastFighters,
    WonAgainstThePointsLeader,
    WonBesideASurvivingFlagship,
    WonAgainstANoteHolder,
    WonInAnAnomaly,
    WonInARivalHome,
    HeldThreeShipsAfterASpaceCombat,
    LostAHomePlanet,
    ElectedByAnAgenda,
    LastToPass,
}

/// Returning and captured units, spec ops counters, feats.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerMisc {
    pub infantry_returning: i16,
    pub spec_ops_returning: i16,
    pub spec_ops_destroyed: u32,
    pub spec_ops_card: u32,
    pub technology_units_returning: SmallList<Unit, 2>,
    /// `Player.captured_units`: (owner of the captured unit, unit type).
    pub captured_units: SmallList<(Seat, Unit), 4>,
    /// `Player.event_feats`: (feat, occurrence number).
    pub event_feats: SmallList<(Feat, u64), 2>,
}
