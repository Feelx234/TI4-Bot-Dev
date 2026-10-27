use crate::containers::{Opt32, SmallList};
use crate::generated::Unit;

/// Round-scoped combat effects. (`fighter_bonus_round`, `bunker_invasion`, `blitz_invasion`,
/// `disable_invasion` are `Player.round_marks` kinds.)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CombatTimers {
    pub combat_bonus_round: Opt32,
    pub rout_round: Opt32,
    pub waylay_barrage_round: Opt32,
    pub retreat_barred_round: Opt32,
    pub extra_die_round: Opt32,
    pub extra_die_unit: Option<Unit>,
    pub munitions_round: Opt32,
    pub ground_roll_suppressed_round: Opt32,
    pub sustained_damage_round: Opt32,
    pub sustained_damage_types: SmallList<Unit, 2>,
    /// `Player.cancel_hits_round`: (round, hits).
    pub cancel_hits_round: Option<(u32, u32)>,
}
