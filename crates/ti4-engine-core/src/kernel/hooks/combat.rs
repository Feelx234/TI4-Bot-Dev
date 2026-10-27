//! Space-combat hooks (`factions/hooks_combat.rs`, 9 slots).

use super::{CombatUnit, Hr, HitSite, AfbExcess, BorrowedStats, ProducedHits, hook_group};
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::CoreState;

hook_group! {
    /// `hooks_combat::CombatHooks`.
    CombatHooks {
        /// Extra hits produced before dice or at a moment of the round (asks; summed). String: `combat`.
        try_plus produced_hits(cx: &mut Cx<'_>, site: HitSite) -> Hr<ProducedHits>;
        /// Whether the unit may use sustain damage; every module must allow it. String: `combat`, `invasion`.
        all may_sustain(s: &CoreState, k: &Kernel, unit: CombatUnit) -> bool;
        /// Immune to direct hits (assigned hits). One module is enough. String: `combat`.
        any direct_hit_immune(s: &CoreState, k: &Kernel, unit: CombatUnit) -> bool;
        /// Immune to ability hits. One module is enough. String: `combat`.
        any ability_hit_immune(s: &CoreState, k: &Kernel, unit: CombatUnit) -> bool;
        /// Anti-fighter-barrage hits that found no fighter left. String: `combat`.
        try_each afb_excess(cx: &mut Cx<'_>, site: AfbExcess) -> Hr<()>;
        /// Space cannon cannot fire at `target` in `system`. String: `combat`.
        any space_cannon_barred(s: &CoreState, k: &Kernel, shooter: Seat, target: Seat, system: System) -> bool;
        /// Fighters count half in the fleet pool for this player. String: `fleet`, `fleet_strength`.
        any fighter_fleet_weight_halves(s: &CoreState, k: &Kernel, who: Seat, unit: Unit) -> bool;
        /// A unit that gains sustain damage from a module. String: `combat`.
        any grants_sustain(s: &CoreState, k: &Kernel, unit: CombatUnit) -> bool;
        /// Stats borrowed from another unit for this player in the system (first module that has them).
        first borrowed_stats(s: &CoreState, k: &Kernel, who: Seat, system: System) -> Option<BorrowedStats>;
    }
}
