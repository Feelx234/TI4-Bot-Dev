//! muaat: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.extra_production` | `extra_production` | Q | seam `extra_production` (a P1 add; the bridge reads the legacy slot too) |
//! | `movement.may_enter_supernova`, `may_pass_through_supernova` | | Q | `movement.rs` (A2, done) |
//! | `component_actions`, `perform_component`, `leader_action`, `use_leader`, `commander_unlocked`, `timing_abilities` | | W | B7 (leaders) |

use ti4_engine_core::generated::UnitBaseType;
use ti4_engine_core::generated::{Faction, System};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, ProdSite, Q, Source};
use ti4_engine_core::state::CoreState;

use super::movement::holds_reactor;
use crate::common::{any_piece_in, is_faction, scope};

const SOURCE: Source = Source::Faction(Faction::TheEmbersOfMuaat);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
    t.seam.extra_production.push_add(SOURCE, Layer::Faction, production);
}

fn production(q: Q<'_>, site: &ProdSite, _: i64) -> Delta {
    match extra_production(q.s, q.k, site.who, site.system) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Magmus Reactor: "Each supernova that contains 1 or more of your units gains the PRODUCTION 5 ability as if it were 1
/// of your units."
pub fn extra_production(s: &CoreState, k: &Kernel, who: Seat, system: System) -> i64 {
    if !is_faction(s, who, Faction::TheEmbersOfMuaat)
        || !holds_reactor(s, who)
        || !scope(k).system(system).is_some_and(|d| d.anomalies.supernova)
    {
        return 0;
    }
    if any_piece_in(s, system, |p| p.owner() == who) { 5 } else { 0 }
}

// -- leader hooks (package F2) ----------------------------------------

/// Magmus: the unlock is recorded by the combat/token reaction; this hook only reports the leader as this module's (`muaat::commander_unlocked`).
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    use ti4_engine_core::state::LeaderStatus;
    (leader == ti4_engine_core::generated::Leader::Magmus).then(|| {
        matches!(
            ti4_engine_core::query::leaders::status(s, who, leader),
            Some(LeaderStatus::Unlocked | LeaderStatus::Readied | LeaderStatus::Exhausted)
        )
    })
}

/// Whether `who` may act on Umbat's text now: the Muaat's own readied agent, or a copy through a readied Ssruu of an agent another seat holds
/// readied or exhausted (`muaat::umbat_available`).
fn umbat_available(s: &CoreState, who: Seat) -> bool {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::query::leaders::status;
    use ti4_engine_core::state::LeaderStatus;
    (is_faction(s, who, Faction::TheEmbersOfMuaat) && status(s, who, Leader::Umbat) == Some(LeaderStatus::Readied))
        || (status(s, who, Leader::Ssruu) == Some(LeaderStatus::Readied)
            && s.players().iter().any(|p| p.seat != who && matches!(p.leaders.status.get(&Leader::Umbat), Some(LeaderStatus::Readied | LeaderStatus::Exhausted))))
}

/// `!muaat::agent_targets(..).is_empty()`: some player has a war sun or flagship in a space area where a production of up to 2 units
/// costing 4 or less would ask something.
fn umbat_has_target(s: &CoreState, k: &Kernel) -> bool {
    let scope = scope(k);
    let roster = crate::cd_registry::yin::timing::roster(scope);
    s.players().iter().any(|p| {
        s.board().systems().any(|(system, cell)| {
            cell.units.as_slice().iter().any(|u| u.owner() == p.seat && scope.unit(u.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Warsun | UnitBaseType::Flagship)))
                && ti4_engine_core::rules::production::probe::would_ask(s, k, &roster, p.seat, system, 2, Some(4))
        })
    })
}

/// Umbat: usable by its holder and some player could produce under it (`muaat::leader_action`).
pub fn leader_action(s: &CoreState, k: &Kernel, who: Seat, leader: ti4_engine_core::generated::Leader) -> Option<bool> {
    (leader == ti4_engine_core::generated::Leader::Umbat).then(|| umbat_available(s, who) && umbat_has_target(s, k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;
    use ti4_engine_core::generated::{Technology, Unit};
    use ti4_engine_core::state::Piece;

    const MUAAT: Faction = Faction::TheEmbersOfMuaat;

    fn supernova() -> System {
        System::ALL.iter().copied().find(|s| s.data().anomalies.supernova).expect("a supernova tile")
    }

    #[test]
    fn a_supernova_with_a_unit_of_the_reactor_holder_has_production_five() {
        let (mut s, k) = table(&[MUAAT, Faction::TheFederationOfSol]);
        let sys = supernova();
        assert_eq!(extra_production(&s, &k, Seat(0), sys), 0, "no reactor");
        s.players_mut().edit(0).tech.owned.insert(Technology::MagmusReactor);
        assert_eq!(extra_production(&s, &k, Seat(0), sys), 0, "no unit there");
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::CarrierI, Seat(0)));
        assert_eq!(extra_production(&s, &k, Seat(0), sys), 5);
        assert_eq!(extra_production(&s, &k, Seat(1), sys), 0, "the other seat");
        let plain = System::ALL.iter().copied().find(|s| !s.data().anomalies.supernova).unwrap();
        s.board_mut().sys_mut(plain).units.push(Piece::new(Unit::CarrierI, Seat(0)));
        assert_eq!(extra_production(&s, &k, Seat(0), plain), 0, "not a supernova");
    }

    #[test]
    fn registering_fills_one_seam_provider() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.seam.extra_production.len(), 1);
    }
}
