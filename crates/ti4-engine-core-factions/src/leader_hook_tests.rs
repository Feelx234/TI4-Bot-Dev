//! The leader hooks (package F2) on hand-built positions: the conditions the bridge's string comparison reaches only through
//! its variants, one at a time, with the answers the string gives. Survivors of the first mutation run drove these.

use ti4_engine_core::generated::{Faction, Leader, Planet, System, Unit, Wormhole};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{Kernel, Sources};
use ti4_engine_core::state::{CoreState, LeaderStatus, MetaFlags, Piece, SysFlags};

use crate::common::testkit::table;

fn kernel() -> Kernel {
    let mut hooks = HookTable::new();
    crate::register_all(&mut hooks);
    Kernel { hooks, sources: Sources(u8::MAX) }
}

fn action(s: &CoreState, k: &Kernel, who: u8, leader: Leader) -> Option<bool> {
    k.hooks.general.leader_action(s, k, Seat(who), leader)
}

fn unlocked(s: &CoreState, k: &Kernel, who: u8, leader: Leader) -> Option<bool> {
    k.hooks.general.commander_unlocked(s, k, Seat(who), leader)
}

fn put(s: &mut CoreState, system: System, who: u8, unit: Unit) {
    let cell = s.board_mut().sys_mut(system);
    cell.units.push(Piece::new(unit, Seat(who)));
    cell.flags.set(SysFlags::TOUCHED, true);
}

fn status(s: &mut CoreState, who: usize, leader: Leader, st: LeaderStatus) {
    s.players_mut().edit(who).leaders.status.insert(leader, st);
}

/// Systems with no printed wormhole and no planet, in id order.
fn plain_systems(n: usize) -> Vec<System> {
    System::ALL.iter().copied().filter(|s| s.data().wormholes.is_empty() && s.data().planets.is_empty() && !matches!(s, System::MalliceLockedNexus | System::MalliceNexus)).take(n).collect()
}

#[test]
fn nekro_malleon_needs_a_readied_card_and_somebody_who_can_pay() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheNekroVirus, Faction::TheFederationOfSol]);
    status(&mut s, 0, Leader::NekroMalleon, LeaderStatus::Readied);
    assert_eq!(action(&s, &k, 0, Leader::NekroMalleon), Some(false), "nobody holds a card or a token");
    for pool in 0..3 {
        let mut t = s.clone();
        let econ = &mut t.players_mut().edit(1).econ;
        match pool {
            0 => econ.tactic_tokens = 1,
            1 => econ.fleet_tokens = 1,
            _ => econ.strategic_tokens = 1,
        }
        assert_eq!(action(&t, &k, 0, Leader::NekroMalleon), Some(true), "one token of pool {pool}");
    }
    let mut t = s.clone();
    t.players_mut().edit(1).cards.hand.push(ti4_engine_core::generated::ActionCard::ALL[0]);
    assert_eq!(action(&t, &k, 0, Leader::NekroMalleon), Some(true), "a card");
    status(&mut t, 0, Leader::NekroMalleon, LeaderStatus::Exhausted);
    assert_eq!(action(&t, &k, 0, Leader::NekroMalleon), Some(false), "an exhausted card");
}

#[test]
fn flayesh_needs_a_specialty_planet_in_a_system_with_the_seats_own_units() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheNekroVirus, Faction::TheFederationOfSol]);
    status(&mut s, 0, Leader::UNITDSGNFLAYESH, LeaderStatus::Unlocked);
    let planet = Planet::ALL.iter().copied().find(|p| !p.data().specialties.is_empty() && p.data().system.is_some()).unwrap();
    let system = planet.data().system.unwrap();
    assert_eq!(action(&s, &k, 0, Leader::UNITDSGNFLAYESH), Some(false), "no units");
    let mut other = s.clone();
    put(&mut other, system, 1, Unit::CruiserI);
    assert_eq!(action(&other, &k, 0, Leader::UNITDSGNFLAYESH), Some(false), "somebody else's units");
    let mut own = s.clone();
    put(&mut own, system, 0, Unit::CruiserI);
    assert_eq!(action(&own, &k, 0, Leader::UNITDSGNFLAYESH), Some(true));
    status(&mut own, 0, Leader::UNITDSGNFLAYESH, LeaderStatus::Locked);
    assert_eq!(action(&own, &k, 0, Leader::UNITDSGNFLAYESH), Some(false), "a locked hero");
}

#[test]
fn flock_migration_needs_a_ship_and_another_system_with_the_token_and_no_foreign_ship() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheArgentFlight, Faction::TheFederationOfSol]);
    let [a, b, ..] = plain_systems(2)[..] else { panic!("two systems") };
    put(&mut s, a, 0, Unit::CruiserI);
    assert_eq!(action(&s, &k, 0, Leader::MirikAunSissiri), Some(false), "no destination");
    s.board_mut().sys_mut(b).cmd.insert(Seat(0));
    assert_eq!(action(&s, &k, 0, Leader::MirikAunSissiri), Some(true));
    let mut foreign = s.clone();
    put(&mut foreign, b, 1, Unit::CruiserI);
    assert_eq!(action(&foreign, &k, 0, Leader::MirikAunSissiri), Some(false), "a foreign ship in the destination");
    let mut ground = s.clone();
    put(&mut ground, b, 1, Unit::InfantryI);
    assert_eq!(action(&ground, &k, 0, Leader::MirikAunSissiri), Some(true), "a foreign ground force is no ship");
    let mut theirs = s.clone();
    theirs.board_mut().sys_mut(a).units.clear();
    put(&mut theirs, a, 1, Unit::CruiserI);
    assert_eq!(action(&theirs, &k, 0, Leader::MirikAunSissiri), Some(false), "only a foreign ship to lift");
    let mut infantry = s.clone();
    infantry.board_mut().sys_mut(a).units.clear();
    put(&mut infantry, a, 0, Unit::InfantryI);
    assert_eq!(action(&infantry, &k, 0, Leader::MirikAunSissiri), Some(false), "only a ground force of the seat");
    let (mut other, _) = table(&[Faction::TheFederationOfSol, Faction::TheArgentFlight]);
    put(&mut other, a, 0, Unit::CruiserI);
    other.board_mut().sys_mut(b).cmd.insert(Seat(0));
    assert_eq!(action(&other, &k, 0, Leader::MirikAunSissiri), Some(false), "not the Argent");
}

#[test]
fn it_feeds_on_carrion_needs_a_cabal_seat_and_a_dimensional_tear() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheVuilraithCabal, Faction::TheFederationOfSol]);
    assert_eq!(action(&s, &k, 0, Leader::ItFeedsOnCarrion), Some(false), "no tear");
    put(&mut s, plain_systems(1)[0], 0, Unit::DimensionalTearI);
    assert_eq!(action(&s, &k, 0, Leader::ItFeedsOnCarrion), Some(true));
    assert_eq!(action(&s, &k, 1, Leader::ItFeedsOnCarrion), Some(false), "not the Cabal's seat");
}

#[test]
fn riftwalker_meian_counts_systems_with_a_wormhole_or_the_seats_units() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol]);
    let [a, b, c, ..] = plain_systems(3)[..] else { panic!("three systems") };
    assert_eq!(action(&s, &k, 0, Leader::RiftwalkerMeian), Some(false), "nothing");
    put(&mut s, a, 0, Unit::CruiserI);
    assert_eq!(action(&s, &k, 0, Leader::RiftwalkerMeian), Some(false), "one system");
    // A token alone makes a system worth naming even with no board entry.
    s.explore_mut().wormhole_tokens.insert(Wormhole::Alpha, b);
    assert_eq!(action(&s, &k, 0, Leader::RiftwalkerMeian), Some(true), "a Creuss token and a unit");
    // Somebody else's units do not count, and the Creuss systems are excluded.
    let (mut t, _) = table(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol]);
    put(&mut t, a, 0, Unit::CruiserI);
    put(&mut t, c, 1, Unit::CruiserI);
    put(&mut t, System::CreussGateCreuss, 0, Unit::CruiserI);
    assert_eq!(action(&t, &k, 0, Leader::RiftwalkerMeian), Some(false), "a foreign unit and an excluded system");
}

#[test]
fn sai_seravus_counts_alpha_and_beta_wormholes_however_they_got_there() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol]);
    let [a, b, c, d, ..] = plain_systems(4)[..] else { panic!("four systems") };
    for system in [a, b, d] {
        put(&mut s, system, 0, Unit::CruiserI);
    }
    s.explore_mut().wormhole_tokens.insert(Wormhole::Alpha, a);
    s.meta_mut().ion_storm = Some((b, Wormhole::Beta));
    put(&mut s, System::MalliceLockedNexus, 0, Unit::CruiserI);
    assert_eq!(unlocked(&s, &k, 0, Leader::SaiSeravus), Some(false), "the Nexus is still locked, the fourth system has none");
    s.meta_mut().flags.set(MetaFlags::NEXUS_UNLOCKED, true);
    assert_eq!(unlocked(&s, &k, 0, Leader::SaiSeravus), Some(true), "a token, a storm face and the opened Nexus");
    let mut delta = s.clone();
    delta.explore_mut().wormhole_tokens.clear();
    delta.explore_mut().wormhole_tokens.insert(Wormhole::Delta, a);
    assert_eq!(unlocked(&delta, &k, 0, Leader::SaiSeravus), Some(false), "a delta wormhole is no alpha or beta");
    let mut foreign = s.clone();
    foreign.board_mut().sys_mut(a).units.clear();
    put(&mut foreign, a, 1, Unit::CruiserI);
    assert_eq!(unlocked(&foreign, &k, 0, Leader::SaiSeravus), Some(false), "a foreign unit does not count");
    let _ = c;
}

#[test]
fn airo_shir_aur_needs_ships_of_two_seats() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheFederationOfSol]);
    status(&mut s, 0, Leader::AiroShirAur, LeaderStatus::Unlocked);
    let [a, b, ..] = plain_systems(2)[..] else { panic!("two systems") };
    put(&mut s, a, 0, Unit::CruiserI);
    assert_eq!(action(&s, &k, 0, Leader::AiroShirAur), Some(false), "nobody else has a ship");
    put(&mut s, b, 1, Unit::InfantryI);
    assert_eq!(action(&s, &k, 0, Leader::AiroShirAur), Some(false), "a ground force is no ship");
    put(&mut s, b, 1, Unit::CruiserI);
    assert_eq!(action(&s, &k, 0, Leader::AiroShirAur), Some(true));
    let (mut alone, _) = table(&[Faction::TheMahactGeneSorcerers]);
    status(&mut alone, 0, Leader::AiroShirAur, LeaderStatus::Unlocked);
    put(&mut alone, a, 0, Unit::CruiserI);
    assert_eq!(action(&alone, &k, 0, Leader::AiroShirAur), Some(false), "the seat itself is not another seat");
}

#[test]
fn xuange_without_a_map_is_locked() {
    let k = kernel();
    let (s, _) = table(&[Faction::TheEmpyrean, Faction::TheFederationOfSol]);
    assert_eq!(unlocked(&s, &k, 0, Leader::Xuange), Some(false));
}

#[test]
fn umbat_is_borrowed_only_from_another_seats_agent() {
    use crate::ce_movement::muaat::query::umbat_available;
    let (mut s, _) = table(&[Faction::TheYssarilTribes, Faction::TheFederationOfSol]);
    status(&mut s, 0, Leader::Ssruu, LeaderStatus::Readied);
    assert!(!umbat_available(&s, Seat(0)), "nobody holds Umbat");
    status(&mut s, 0, Leader::Umbat, LeaderStatus::Readied);
    assert!(!umbat_available(&s, Seat(0)), "the seat's own card is not another seat's");
    status(&mut s, 1, Leader::Umbat, LeaderStatus::Exhausted);
    assert!(umbat_available(&s, Seat(0)), "an exhausted agent of another seat can be copied");
    status(&mut s, 1, Leader::Umbat, LeaderStatus::Locked);
    assert!(!umbat_available(&s, Seat(0)), "a locked card cannot");
}

#[test]
fn gurno_aggero_needs_a_dock_and_the_card() {
    let k = kernel();
    let (mut s, _) = table(&[Faction::TheClanOfSaar, Faction::TheFederationOfSol]);
    let system = plain_systems(1)[0];
    assert_eq!(action(&s, &k, 0, Leader::GurnoAggero), Some(false), "no card, no dock");
    status(&mut s, 0, Leader::GurnoAggero, LeaderStatus::Unlocked);
    put(&mut s, system, 0, Unit::CruiserI);
    assert_eq!(action(&s, &k, 0, Leader::GurnoAggero), Some(false), "a ship is no dock");
    put(&mut s, system, 0, Unit::SpaceDockI);
    assert_eq!(action(&s, &k, 0, Leader::GurnoAggero), Some(true));
}
