//! muaat: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.extra_production` | `extra_production` | Q | seam `extra_production` (a P1 add; the bridge reads the legacy slot too) |
//! | `movement.may_enter_supernova`, `may_pass_through_supernova` | | Q | `movement.rs` (A2, done) |
//! | `component_actions`, `perform_component`, `leader_action`, `use_leader`, `commander_unlocked`, `timing_abilities` | | W | B7 (leaders) |

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
