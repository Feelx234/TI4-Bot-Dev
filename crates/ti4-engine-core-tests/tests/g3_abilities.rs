//! Lane G3: the loud gap rows turned into rules of the core table (`ti4_engine_core_factions::silent::g3`), part 2. The lockstep against the string is in
//! `ti4-engine` (`invasion_live_g3_tests`, `silent3_live_tests`); these tests pin the rules.

use ti4_engine_core::generated::{Faction, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{EventKind, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_before_movement(&mut t);
    ti4_engine_core_factions::silent::register_abilities(&mut t);
    t
}

fn rig(scenario: Scenario, state_edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    state_edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(table());
    r.run();
    r
}

/// A plain system with at least two planets, and its planets.
fn two_planet_system() -> (System, Vec<Planet>) {
    let system = ti4_engine_core_tests::scenario::plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a plain system with two planets");
    (system, system.data().planets.to_vec())
}

fn units_of(r: &Rig<EmitEvent>, planet: Planet, seat: u8, base: UnitBaseType) -> usize {
    board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(seat) && p.kind().data().base_type == base).count()
}

fn goods(r: &Rig<EmitEvent>, seat: u8) -> i16 {
    r.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

// -- Scavenger Zeta -----------------------------------------------------------------------------------------------------------

fn gained(seat: u8, system: System, planet: Planet) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::PlanetControlGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::Planet, Val::Planet(planet)).with(Key::System, Val::System(system)))
}

#[test]
fn scavenger_zeta_costs_a_trade_good_and_places_a_mech_on_the_planet_gained() {
    covers!("silent::g3::zeta::ZETA");
    let (system, planets) = two_planet_system();
    let scenario = || Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 0).goods(0, 2);
    let mut r = rig(scenario(), |_| {}, gained(0, system, planets[0]));
    // The window lists Scavenge (mandatory) and the deploy together: Scavenge first (it pays the good), then the optional deploy.
    r.pick(0);
    assert!(matches!(r.asked().ask, ti4_engine_core::choice::Ask::Reaction(_)), "{}", r.transcript());
    let use_it = r.opts().iter().position(|o| *o != Opt::Decline).expect("the deploy");
    r.pick(use_it);
    assert!(r.is_done(), "{}", r.transcript());
    assert_eq!(units_of(&r, planets[0], 0, UnitBaseType::Mech), 1);
    assert_eq!(goods(&r, 0), 2, "2 + 1 from Scavenge - 1 for the mech");
    // Declined: nothing is placed or paid.
    let mut declined = rig(scenario(), |_| {}, gained(0, system, planets[0]));
    declined.pick(0);
    declined.decline();
    assert!(declined.is_done());
    assert_eq!((units_of(&declined, planets[0], 0, UnitBaseType::Mech), goods(&declined, 0)), (0, 3));
}

#[test]
fn scavenger_zeta_needs_the_gainer_a_saar_seat_and_a_mech_in_the_box() {
    covers!("silent::g3::zeta::ZETA");
    let (system, planets) = two_planet_system();
    // Another seat gains the planet: the Saar is not asked.
    let other = rig(Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 1).goods(0, 2).goods(1, 2), |_| {}, gained(1, system, planets[0]));
    assert!(other.is_done(), "{}", other.transcript());
    assert_eq!(units_of(&other, planets[0], 1, UnitBaseType::Mech), 0);
    // A seat of another faction has no Scavenger Zeta.
    let plain = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheClanOfSaar]).control(planets[0], 0).goods(0, 2), |_| {}, gained(0, system, planets[0]));
    assert!(plain.is_done(), "{}", plain.transcript());
    // The four mechs stand elsewhere: the box is empty.
    let mut scenario = Scenario::new(&[Faction::TheClanOfSaar, Faction::TheEmiratesOfHacan]).control(planets[0], 0).goods(0, 2);
    scenario = scenario.unit(system, Some(planets[1]), Unit::ScavengerZeta, 0, 4);
    let spent = rig(scenario, |_| {}, gained(0, system, planets[0]));
    assert!(spent.is_done(), "no mech left to place: {}", spent.transcript());
    assert_eq!(units_of(&spent, planets[0], 0, UnitBaseType::Mech), 0);
}
