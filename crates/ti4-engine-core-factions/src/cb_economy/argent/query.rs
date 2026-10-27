//! argent: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `vote_bonus` | `vote_bonus` | Q | `general.vote_bonus` |
//! | `cards.votes_first` | `votes_first` | Q | `cards.votes_first` |
//! | `combat.space_cannon_barred` | `space_cannon_barred` | Q | `combat.space_cannon_barred` |
//! | `economy.extra_production_planet` | `extra_production_planet` | Q | `economy.extra_production_planet` |
//! | `economy.production_destinations` | `agent_production_destinations` | Q | `economy.production_destinations` |
//! | `combat.afb_excess` | `afb_excess` | W | `offer.rs` (empty until the combat window owner ports the ask) |
//! | `direct.argent_clear_agent_production_destinations` | `clear_agent_production_destinations` | O | `observe.rs` |
//! | movement `blocks_passage`, `free_cargo` | | Q | `movement.rs` (A2, done) |
//! | `leader_action`, `use_leader`, `timing_abilities`, `commander_unlocked` | | W | B7 (leaders) |

use ti4_engine_core::generated::{Faction, Planet, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::economy::Destinations;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::CoreState;

use crate::common::{flagship_has_text, has_tech_text, is_faction, mark, scope, system as system_atom};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.vote_bonus.push(vote_bonus);
    t.cards.votes_first.push(votes_first);
    t.combat.space_cannon_barred.push(space_cannon_barred);
    t.economy.extra_production_planet.push(extra_production_planet);
    t.economy.production_destinations.push(agent_production_destinations);
}

fn is_argent(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheArgentFlight)
}

/// Zeal: "cast 1 additional vote for each player in the game including you."
pub fn vote_bonus(s: &CoreState, who: Seat) -> i64 {
    if is_argent(s, who) { i64::try_from(s.players().iter().count()).unwrap_or(0) } else { 0 }
}

/// Zeal: "You always vote first during the agenda phase."
pub fn votes_first(s: &CoreState, who: Seat) -> bool {
    is_argent(s, who)
}

/// Quetzecoatl: "Other players cannot use SPACE CANNON against your ships in this system."
pub fn space_cannon_barred(s: &CoreState, _: &Kernel, shooter: Seat, target: Seat, system: System) -> bool {
    shooter != target
        && s.board().sys(system).units.as_slice().iter().any(|u| u.owner() == target && flagship_has_text(s, target, u.kind(), Unit::Quetzecoatl))
}

/// Whether `who` has a structure on the planet.
fn structure_on(s: &CoreState, k: &Kernel, who: Seat, planet: Planet) -> bool {
    let scope = scope(k);
    s.board().planet(planet).units.as_slice().iter().any(|u| u.owner() == who && scope.unit(u.kind()).is_some_and(|d| d.structure))
}

/// Aerie Hololattice: "Each planet that contains 1 or more of your structures gains the PRODUCTION 1 ability
/// as if it were a unit."
pub fn extra_production_planet(s: &CoreState, k: &Kernel, who: Seat, _: System, planet: Planet) -> i64 {
    i64::from(has_tech_text(s, who, Technology::AerieHololattice) && structure_on(s, k, who, planet))
}

/// The unit the string engine names by its base type (`catalogue.get("infantry")`): the plain unit of that base.
fn plain_unit(base: UnitBaseType) -> Option<Unit> {
    Unit::ALL.into_iter().find(|u| {
        let d = u.data();
        d.faction.is_none() && d.base_type == base && d.upgrades_from.is_none()
    })
}

/// Destinations granted by Argent's agent for the in-progress production: the one-build mark of the
/// producing system, ground forces only, planets still controlled, not demilitarized, not a space station.
pub fn agent_production_destinations(s: &CoreState, k: &Kernel, who: Seat, system: System, base: UnitBaseType) -> Destinations {
    let mut out = Destinations::new();
    let ground = matches!(base, UnitBaseType::Infantry | UnitBaseType::Mech);
    if !ground || !plain_unit(base).is_some_and(|u| scope(k).unit(u).is_some_and(|d| !d.structure)) {
        return out;
    }
    let Some(ti4_engine_core::keys::MarkVal::List(entries)) = mark(s, MarkKey::ArgentProduction(who, system)) else { return out };
    let blanked = ti4_engine_core::query::reach::blanked(s);
    for entry in entries {
        let ti4_engine_core::keys::MarkVal::Rec(fields) = entry else { continue };
        let [target, planet] = fields.as_slice() else { continue };
        let (Some(target), Some(planet)) = (system_atom(target), super::super::planet(planet)) else { continue };
        let controlled = ti4_engine_core::query::board::control(s, planet) == Some(who);
        if target != system
            && controlled
            && !ti4_engine_core::query::laws::planet_is_demilitarized(s, blanked, planet)
            && !planet.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Spacestation)
        {
            out.push((target, Some(planet)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::keys::{Atom, MarkVal};

    const A: Faction = Faction::TheArgentFlight;
    const SOL: Faction = Faction::TheFederationOfSol;

    #[test]
    fn zeal_is_the_argent_seats_alone() {
        let (s, _) = table(&[A, SOL, SOL]);
        assert_eq!(vote_bonus(&s, Seat(0)), 3, "one extra vote per player, the Argent included");
        assert_eq!(vote_bonus(&s, Seat(1)), 0);
        assert!(votes_first(&s, Seat(0)));
        assert!(!votes_first(&s, Seat(2)));
    }

    #[test]
    fn the_flagship_bars_space_cannon_of_the_other_players_only() {
        let (mut s, k) = table(&[A, SOL]);
        let sys = ti4_engine_core::ids::system_at(30);
        s.board_mut().sys_mut(sys).units.push(ti4_engine_core::state::Piece::new(Unit::Quetzecoatl, Seat(0)));
        assert!(space_cannon_barred(&s, &k, Seat(1), Seat(0), sys));
        assert!(!space_cannon_barred(&s, &k, Seat(0), Seat(0), sys), "your own space cannon is not barred");
        assert!(!space_cannon_barred(&s, &k, Seat(0), Seat(1), sys), "the flagship must belong to the target");
    }

    #[test]
    fn hololattice_gives_a_planet_with_a_structure_one_production() {
        let (mut s, k) = table(&[A, SOL]);
        let sys = ti4_engine_core::ids::system_at(30);
        s.players_mut().edit(0).tech.owned.insert(Technology::AerieHololattice);
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::Abaddon), 0, "no structure");
        s.board_mut().planet_mut(Planet::Abaddon).units.push(ti4_engine_core::state::Piece::new(Unit::PDSI, Seat(0)));
        assert_eq!(extra_production_planet(&s, &k, Seat(0), sys, Planet::Abaddon), 1);
        assert_eq!(extra_production_planet(&s, &k, Seat(1), sys, Planet::Abaddon), 0, "the other seat owns neither");
    }

    #[test]
    fn the_agent_grants_ground_forces_a_controlled_remote_planet() {
        let (mut s, k) = table(&[A, SOL]);
        let (home, remote) = (ti4_engine_core::ids::system_at(30), ti4_engine_core::ids::system_at(31));
        let grant = MarkVal::List(vec![MarkVal::Rec(vec![MarkVal::Atom(Atom::System(remote)), MarkVal::Atom(Atom::Planet(Planet::Abaddon))])]);
        s.marks_mut().tail.edit().insert(MarkKey::ArgentProduction(Seat(0), home), grant);
        assert!(agent_production_destinations(&s, &k, Seat(0), home, UnitBaseType::Infantry).is_empty(), "not controlled");
        s.board_mut().planet_mut(Planet::Abaddon).control = Some(Seat(0));
        let found = agent_production_destinations(&s, &k, Seat(0), home, UnitBaseType::Infantry);
        assert_eq!(found.as_slice(), &[(remote, Some(Planet::Abaddon))]);
        assert!(agent_production_destinations(&s, &k, Seat(0), home, UnitBaseType::Pds).is_empty(), "structures are not ground forces");
        assert!(agent_production_destinations(&s, &k, Seat(0), home, UnitBaseType::Carrier).is_empty());
        assert!(agent_production_destinations(&s, &k, Seat(0), remote, UnitBaseType::Infantry).is_empty(), "no mark for that system");
    }

    #[test]
    fn registering_fills_the_five_economy_slots() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.registered(), 5);
    }
}
