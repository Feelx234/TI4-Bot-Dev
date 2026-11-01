//! Lane E2: Xxcha's Peace Accords (`rules::peace_accords`), the annexation window after a Diplomacy ability. The lockstep against the string
//! (`faction_abilities::strategy_resolved`) is `ti4-engine`'s `peace_accords_live_tests`; these tests pin the candidates, the answer and what
//! the window leaves to its caller.

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::rules::peace_accords::{annexable, has_peace_accords, PeaceAccords};
use ti4_engine_core::state::{PlanetFlags, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn annex(system: System, planet: Planet) -> Opt {
    Opt::Strategy(SOpt::Annex(system, planet))
}

/// The Xxcha at seat 0 and the Arborec at seat 1 on a ring of seven plain systems (the first is the centre, the rest its neighbours).
fn table() -> Scenario {
    Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheArborec]).map_ring(7)
}

fn centre() -> System {
    plain_systems()[0]
}

fn offered(rig: &Rig<PeaceAccords>) -> Vec<(System, Planet)> {
    rig.opts().iter().filter_map(|o| if let Opt::Strategy(SOpt::Annex(s, p)) = o { Some((*s, *p)) } else { None }).collect()
}

#[test]
fn the_empty_uncontrolled_planets_in_or_next_to_the_seats_systems_are_offered_in_the_strings_order_with_a_decline() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    let mut rig = Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run();
    // The systems by id (integer order = text order), the planets of each in the order of the system record; the centre's own other planets and
    // the six neighbours (the seven systems are all in or next to the centre), no planet the seat holds.
    let mut want: Vec<(System, Planet)> = plain_systems().iter().take(7).flat_map(|s| s.data().planets.iter().map(|p| (*s, *p)).collect::<Vec<_>>()).filter(|(_, p)| *p != held).collect();
    want.sort_by_key(|(s, _)| *s);
    assert_eq!(offered(&rig), want);
    assert!(want.len() >= 6, "{want:?}");
    assert_eq!(rig.opts().last(), Some(&Opt::Decline));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::PeaceAccords { controlled: 1 }), "the preview counts the planets held now");
    assert_eq!(rig.asked().actor, Seat(0));
    rig.assert_options_sound();
}

#[test]
fn a_planet_with_units_a_rival_planet_mecatol_rex_and_a_space_station_are_not_offered() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    // A system of the ring with units on its planet, and one a rival holds.
    let ring: Vec<System> = plain_systems().into_iter().skip(1).take(6).collect();
    let (occupied, taken) = (ring[0].data().planets[0], ring[1].data().planets[0]);
    let sc = table().control(held, 0).unit(ring[0], Some(occupied), Unit::InfantryI, 1, 1).control(taken, 1);
    let state = sc.state();
    let got = annexable(&state, Seat(0));
    assert!(!got.iter().any(|(_, p)| *p == occupied || *p == taken || *p == held), "{got:?}");
    assert!(got.iter().any(|(s, _)| *s == ring[2]), "an empty neighbour is offered");
    // Own units on the planet make it not empty too (a planet of the seat's own system with its own infantry).
    let own = ring[2].data().planets[0];
    let state = table().control(held, 0).unit(ring[2], Some(own), Unit::InfantryI, 0, 1).state();
    assert!(!annexable(&state, Seat(0)).iter().any(|(_, p)| *p == own), "the seat's own units are units");
    // Mecatol Rex and a space station: a map of the centre, Mecatol Rex and the Bellatrix / Tsion Station system beside it.
    let station = System::BellatrixTsionStation;
    let state = Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheArborec]).map_from(&[centre(), System::MecatolRex112, station]).control(held, 0).state();
    let got = annexable(&state, Seat(0));
    assert!(!got.iter().any(|(_, p)| *p == Planet::MecatolRex), "Mecatol Rex is never annexed: {got:?}");
    assert!(!got.iter().any(|(_, p)| *p == Planet::TsionStation), "a space station is not a planet to annex: {got:?}");
    assert!(got.iter().any(|(s, p)| *s == station && *p == Planet::Bellatrix), "{got:?}");
}

#[test]
fn only_the_neighbours_of_a_system_the_seat_holds_a_planet_in_are_in_reach() {
    covers!("rules::peace_accords::PeaceAccords");
    // The seat holds the planet of a ring system (a system with one planet offers nothing itself): the centre and its two ring neighbours are in reach, the three systems across are not.
    let ring: Vec<System> = plain_systems().into_iter().skip(1).take(6).collect();
    let held = ring[0].data().planets[0];
    let state = table().control(held, 0).state();
    let systems: std::collections::BTreeSet<System> = annexable(&state, Seat(0)).iter().map(|(s, _)| *s).collect();
    // (The ring of the spiral is in coordinate order, not in angle: the neighbours are read from the map.)
    let near: std::collections::BTreeSet<System> = state.galaxy().unwrap().adjacent(ring[0]).iter().map(ti4_engine_core::ids::system_at).collect();
    assert_eq!(near.len(), 3, "the centre and two ring systems: {near:?}");
    assert_eq!(systems, near, "the systems next to the one the seat holds (the system itself has no other planet)");
    assert!(near.contains(&centre()));
    assert!(ring.iter().skip(1).any(|s| !systems.contains(s)), "the systems across are out of reach");
    // A seat without a planet reaches nothing.
    assert!(annexable(&table().state(), Seat(0)).is_empty());
}

#[test]
fn the_answer_gives_control_and_leaves_the_exploration_to_the_caller() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    let ring1 = plain_systems()[1];
    let target = ring1.data().planets[0];
    let mut rig = Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run().pick_opt(annex(ring1, target));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.annexed, Some((ring1, target)));
    assert!(!rig.driver.window.unsupported);
    assert_eq!(rig.state.board().planet(target).control, Some(Seat(0)));
    assert!(rig.state.board().sys(ring1).flags.has(SysFlags::TOUCHED));
    assert!(!rig.state.board().planet(target).flags.has(PlanetFlags::EXHAUSTED), "a planet taken is ready (the string only sets its controller)");
    assert!(rig.state.ledger().events.is_empty(), "no event: the string takes the planet with a plain `set_control`");
}

#[test]
fn a_decline_changes_nothing_and_a_planet_that_was_not_offered_is_illegal() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    let mut rig = Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run().decline();
    assert!(rig.is_done() && rig.driver.window.annexed.is_none());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    let mut rig = Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run();
    rig.expect_illegal(Pick::Opt(annex(centre(), held)));
}

#[test]
fn only_a_seat_with_the_ability_and_a_map_is_asked() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    // Another faction: nothing is asked.
    let mut rig = Rig::new(Scenario::new(&[Faction::TheArborec, Faction::TheXxchaKingdom]).map_ring(7).control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run();
    assert!(rig.is_done());
    assert!(has_peace_accords(&table().state(), Seat(0)) && !has_peace_accords(&table().state(), Seat(1)));
    // A position without a galaxy ("in or next to" needs the map).
    let mut state = table().control(held, 0).state();
    state.loaded = ti4_engine_core::state::SliceSet::of_without(state.loaded, ti4_engine_core::state::Slice::Map);
    assert!(annexable(&state, Seat(0)).is_empty());
}

#[test]
fn a_destroyed_planet_is_offered_and_keeps_no_controller_like_the_string() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    let ring1 = plain_systems()[1];
    let target = ring1.data().planets[0];
    let mut state = table().control(held, 0).state();
    state.board_mut().planet_mut(target).flags.set(PlanetFlags::PURGED, true);
    let mut rig = Rig::from_parts(state, ti4_engine_core_tests::kernel_all(), PeaceAccords::new(Seat(0)));
    rig.run();
    assert!(offered(&rig).contains(&(ring1, target)), "the string offers it");
    rig.pick_opt(annex(ring1, target));
    assert_eq!(rig.state.board().planet(target).control, None, "`set_control` takes no purged planet");
    assert_eq!(rig.driver.window.annexed, Some((ring1, target)));
}

#[test]
fn integrated_economy_and_the_thunders_edge_planet_are_left_to_the_caller_as_unsupported() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    let ring1 = plain_systems()[1];
    let target = ring1.data().planets[0];
    let mut rig = Rig::new(table().control(held, 0).tech(0, Technology::IntegratedEconomy), PeaceAccords::new(Seat(0)));
    rig.run().pick_opt(annex(ring1, target));
    assert!(rig.driver.window.unsupported, "Integrated Economy produces on the planet gained: not carried");
    let mut rig = Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0)));
    rig.run().pick_opt(annex(ring1, target));
    assert!(!rig.driver.window.unsupported);
}

#[test]
fn the_window_holds_up_under_the_standard_smoke() {
    covers!("rules::peace_accords::PeaceAccords");
    let held = centre().data().planets[0];
    smoke_quiet(|| Rig::new(table().control(held, 0), PeaceAccords::new(Seat(0))));
}
