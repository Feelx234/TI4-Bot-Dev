//! W10 (lane 8): `rules::hero_windows`, the heroes composed from windows and kernel effects. UNIT.DSGN.FLAYESH (Nekro hero) here.

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::generated::Planet;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::rules::hero_windows::{Devourable, NekroHero};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

const SOURCE: Code = Code(7);

/// Two planets in two different systems, in system order.
fn two() -> [(System, Planet); 2] {
    let mut found: Vec<(System, Planet)> = Vec::new();
    for s in plain_systems() {
        if let Some(p) = s.data().planets.first()
            && s.data().planets.len() == 1
        {
            found.push((s, *p));
        }
        if found.len() == 2 {
            break;
        }
    }
    found.sort_unstable();
    [found[0], found[1]]
}

fn target(spot: (System, Planet), goods: i32, techs: &[Technology]) -> Devourable {
    Devourable { system: spot.0, planet: spot.1, goods, techs: techs.to_vec() }
}

fn scenario() -> (Scenario, [(System, Planet); 2]) {
    let [a, b] = two();
    let sc = Scenario::new(&[Faction::TheNekroVirus, Faction::TheArborec])
        .map_from(&[a.0, b.0])
        .unit(a.0, None, Unit::CruiserI, 0, 1)
        .unit(b.0, None, Unit::CruiserI, 0, 1)
        .unit(a.0, Some(a.1), Unit::InfantryI, 1, 2)
        .unit(a.0, Some(a.1), Unit::PDSI, 1, 1)
        .unit(a.0, Some(a.1), Unit::InfantryI, 0, 1)
        .unit(b.0, Some(b.1), Unit::InfantryI, 1, 1)
        .goods(0, 1);
    (sc, [a, b])
}

fn hero(sc: Scenario, planets: Vec<Devourable>) -> Rig<NekroHero> {
    Rig::new(sc, NekroHero::new(Seat(0), SOURCE, planets))
}

fn units_on(rig: &Rig<NekroHero>, planet: Planet, seat: u8) -> usize {
    rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count()
}

fn staged_rows(rig: &Rig<NekroHero>) -> usize {
    rig.state.marks().tail.iter().filter(|(k, _)| matches!(k, MarkKey::PrivateStagedGround(_))).count()
}

#[test]
fn a_single_planet_is_devoured_unasked_when_one_technology_matches() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let mut rig = hero(sc, vec![target(a, 3, &[Technology::GravityDrive])]);
    rig.run();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.used && !rig.driver.window.unsupported);
    assert_eq!(units_on(&rig, a.1, 1), 0, "every other seat's unit on the planet is gone, the structure too");
    assert_eq!(units_on(&rig, a.1, 0), 1, "the seat's own unit stays");
    assert_eq!(units_on(&rig, b.1, 1), 1, "the other planet is untouched");
    assert_eq!(staged_rows(&rig), 2, "the two infantry are staged as destroyed, the structure is not");
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 1 + 3);
    assert!(p.tech.owned.contains(Technology::GravityDrive));
    assert_eq!(rig.state.pending().staged.len(), 1, "the goods are announced");
}

#[test]
fn several_planets_and_technologies_are_asked_and_nothing_changes_before_the_last_answer() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let techs = [Technology::GravityDrive, Technology::SarweenTools, Technology::PlasmaScoring];
    let mut rig = hero(sc, vec![target(a, 3, &techs), target(b, 2, &techs[1..])]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlayeshPlanet)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Devour(a.0, a.1)), Opt::Leaders(LOpt::Devour(b.0, b.1))]);
    rig.assert_options_sound();
    assert_eq!(delta(rig.start_state(), &rig.state), "", "nothing changed yet");
    rig.pick_opt(Opt::Leaders(LOpt::Devour(b.0, b.1)));
    assert!(matches!(rig.asked().ask, Ask::Leaders(LAsk::FlayeshTech)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Gain(Technology::SarweenTools)), Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring))], "the technologies of the chosen planet");
    assert_eq!(delta(rig.start_state(), &rig.state), "", "still nothing");
    rig.pick_opt(Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring)));
    assert!(rig.is_done());
    assert_eq!(units_on(&rig, b.1, 1), 0);
    assert_eq!(units_on(&rig, a.1, 1), 3, "the planet not chosen keeps its units");
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert!(p.tech.owned.contains(Technology::PlasmaScoring) && !p.tech.owned.contains(Technology::SarweenTools));
    assert_eq!(p.econ.trade_goods, 1 + 2, "the goods of the chosen planet");
}

#[test]
fn a_planet_nobody_else_holds_still_gives_the_goods_and_a_planet_without_a_technology_gives_none() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, _]) = scenario();
    let mut rig = hero(sc, vec![target(a, 4, &[])]);
    rig.state.board_mut().planet_mut(a.1).units.retain(|p| p.owner() == Seat(0));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(staged_rows(&rig), 0);
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 5);
    assert!(p.tech.owned.is_empty(), "no technology matched");
}

#[test]
fn no_planet_means_the_card_is_not_used() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, _) = scenario();
    let mut rig = hero(sc, Vec::new());
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn an_answer_outside_the_offer_is_refused() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, b]) = scenario();
    let techs = [Technology::GravityDrive, Technology::SarweenTools];
    let mut rig = hero(sc, vec![target(a, 3, &techs), target(b, 2, &techs)]);
    rig.run();
    let stranger = system_of(Planet::MecatolRex);
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Devour(stranger, Planet::MecatolRex))));
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Gain(Technology::GravityDrive))));
    rig.pick_opt(Opt::Leaders(LOpt::Devour(a.0, a.1)));
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Gain(Technology::PlasmaScoring))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
}

#[test]
fn a_galvanized_ground_force_that_falls_makes_the_window_unsupported() {
    covers!("rules::hero_windows::NekroHero");
    let (sc, [a, _]) = scenario();
    let mut rig = hero(sc, vec![target(a, 3, &[Technology::GravityDrive])]);
    for piece in rig.state.board_mut().planet_mut(a.1).units.as_mut_slice() {
        if piece.owner() == Seat(1) && piece.kind() == Unit::InfantryI {
            *piece = piece.with_galvanized(true);
        }
    }
    rig.run();
    assert!(rig.driver.window.unsupported, "the staged row cannot hold the galvanize tail");
}
