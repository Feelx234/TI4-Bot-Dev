//! ghost: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.production_cost_reduction` | `ghost::production_cost_reduction` | Q | `economy.production_cost_reduction` (the `production_discount` bridge reads it) |
//! | `economy.extra_production` | `ghost::extra_production` | Q | `economy.extra_production` (the `extra_production` bridge reads it) |
//! | `movement.{extra_wormholes, linked_systems, move_bonus}` | same names | Q | `movement.rs` (A2, done) |
//! | `combat.produced_hits` | `ghost::produced_hits` | W | `offer.rs` |
//! | component actions, leaders, timing abilities, `commander_unlocked` | | W | B7 |

use ti4_engine_core::generated::Wormhole;
use ti4_engine_core::generated::{Breakthrough, System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, MetaFlags};

use crate::common::{flagship_has_text, holds_breakthrough, scope};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
    t.economy.extra_production.push(extra_production);
    t.economy.production_cost_reduction.push(production_cost_reduction);
}

/// Physical wormholes in `system`: the printed ones, the tokens, the ion storm's face, the unlocked Nexus's two, and
/// Hil Colish's delta (a printed alpha plus an alpha token are two).
fn wormhole_count(s: &CoreState, system: System) -> i64 {
    let printed = system.data().wormholes.len() as i64;
    let tokens = s.explore().wormhole_tokens.iter().filter(|(_, at)| **at == system).count() as i64;
    let storm = i64::from(s.meta().ion_storm.is_some_and(|(at, _)| at == system));
    let nexus = i64::from(s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED) && system == System::MalliceLockedNexus) * 2;
    let hil_colish = i64::from(
        board::space_units(s, system).iter().any(|p| flagship_has_text(s, p.owner(), p.kind(), Unit::HilColish)),
    );
    printed + tokens + storm + nexus + hil_colish
}

/// Particle Synthesis: each wormhole in a system that contains your ships gains PRODUCTION 1.
fn extra_production(s: &CoreState, k: &Kernel, who: Seat, system: System) -> i64 {
    let scope = scope(k);
    let ships = board::space_units(s, system).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ship));
    if holds_breakthrough(s, who, Breakthrough::ParticleSynthesis) && ships { wormhole_count(s, system) } else { 0 }
}

/// Particle Synthesis: reduce the combined cost of units produced in a system by 1 for each wormhole in it.
fn production_cost_reduction(s: &CoreState, who: Seat, system: System) -> i64 {
    if holds_breakthrough(s, who, Breakthrough::ParticleSynthesis) { wormhole_count(s, system) } else { 0 }
}

// -- leader hooks (package F2) ----------------------------------------

/// Whether `system` holds a wormhole of a kind `pred` accepts **now**, from the position alone: printed on the tile, the Creuss tokens, the
/// ion storm's face, the opened Wormhole Nexus and the wormholes the modules' pieces carry (`hooks_movement::wormholes_at`).
fn holds_wormhole(s: &CoreState, k: &Kernel, system: System, pred: impl Fn(Wormhole) -> bool) -> bool {
    system.data().wormholes.iter().any(|w| pred(*w))
        || s.explore().wormhole_tokens.iter().any(|(w, at)| *at == system && pred(*w))
        || s.meta().ion_storm.is_some_and(|(at, w)| at == system && pred(w))
        || (s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED) && system == System::MalliceLockedNexus && (pred(Wormhole::Alpha) || pred(Wormhole::Beta)))
        || k.hooks.movement.extra_wormholes(s).iter().any(|(at, w)| *at == system && pred(*w))
}

/// `hooks_movement::has_alpha_or_beta`.
fn has_alpha_or_beta(s: &CoreState, k: &Kernel, system: System) -> bool {
    holds_wormhole(s, k, system, |w| matches!(w, Wormhole::Alpha | Wormhole::Beta))
}

/// Sai Seravus, unlock: "Have units in 3 systems that contain alpha or beta wormholes." (Only systems the string board has an entry for.)
pub fn commander_unlocked(s: &CoreState, k: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    (leader == ti4_engine_core::generated::Leader::SaiSeravus).then(|| {
        System::ALL
            .iter()
            .filter(|system| ti4_engine_core::query::leader_reads::has_entry(s, **system))
            .filter(|system| has_alpha_or_beta(s, k, **system) && crate::common::any_piece_in(s, **system, |p| p.owner() == who))
            .count()
            >= 3
    })
}

/// `ghost::hero_excluded`: the Creuss systems, the Wormhole Nexus and the Fracture.
fn hero_excluded(system: System) -> bool {
    matches!(system, System::CreussGateCreuss | System::CreussCreuss | System::MalliceLockedNexus | System::MalliceNexus)
        || system.data().tile_back == Some(ti4_engine_core::generated::TileBack::Fracture)
}

/// Riftwalker Meian: what the offer can know without a map: systems of the board record or holding a Creuss token that hold a wormhole or
/// the owner's units, not excluded; at least two (`ghost::leader_action`).
pub fn leader_action(s: &CoreState, k: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    (leader == ti4_engine_core::generated::Leader::RiftwalkerMeian).then(|| {
        System::ALL
            .iter()
            .filter(|system| ti4_engine_core::query::leader_reads::has_entry(s, **system) || s.explore().wormhole_tokens.iter().any(|(_, at)| at == *system))
            .filter(|system| !hero_excluded(**system))
            .filter(|system| holds_wormhole(s, k, **system, |_| true) || crate::common::any_piece_in(s, **system, |p| p.owner() == who))
            .count()
            >= 2
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cc_ground::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::state::Piece;

    #[test]
    fn particle_synthesis_counts_wormholes_and_needs_ships_for_production() {
        let (mut s, _) = table(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol]);
        let system = System::ALL.iter().copied().find(|sys| !sys.data().wormholes.is_empty()).expect("a wormhole system");
        let wormholes = system.data().wormholes.len() as i64;
        let k = Kernel { sources: ti4_engine_core::kernel::Sources(u8::MAX), ..Kernel::plain() };
        assert_eq!(production_cost_reduction(&s, Seat(0), system), 0, "no breakthrough");
        s.players_mut().edit(0).cards.breakthrough = Some(Breakthrough::ParticleSynthesis);
        assert_eq!(production_cost_reduction(&s, Seat(0), system), wormholes);
        assert_eq!(production_cost_reduction(&s, Seat(1), system), 0);
        assert_eq!(extra_production(&s, &k, Seat(0), system), 0, "no ships there");
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        assert_eq!(extra_production(&s, &k, Seat(0), system), wormholes);
        s.explore_mut().wormhole_tokens.insert(ti4_engine_core::generated::Wormhole::Alpha, system);
        assert_eq!(production_cost_reduction(&s, Seat(0), system), wormholes + 1, "a token adds a wormhole");
    }
}
