//! Lane D (KV3): the placement riders of `rules::agenda::close::Close` (Construction, Diplomacy, Warfare): the place is forced when there is
//! one and asked when there are several, and the reward lands there. The string is `action_cards::asked_rider_payoff`.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Opt as AOpt, Rider};
use ti4_engine_core::generated::{Agenda, Planet, Source, System, Unit};
use ti4_engine_core::keys::{Prediction, PredictionTag};
use ti4_engine_core::rules::agenda::close::{Close, Facts};
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn two_planets() -> (Planet, Planet) {
    let mut found: Vec<Planet> = Vec::new();
    for system in plain_systems() {
        for p in system.data().planets.iter().copied() {
            if p.data().source == Source::Base && !p.data().legendary && !found.contains(&p) && found.iter().all(|f| system_of(*f) != system_of(p)) {
                found.push(p);
            }
        }
        if found.len() == 2 {
            break;
        }
    }
    (found[0], found[1])
}

fn facts() -> Facts {
    Facts {
        trade_rider: Code(1),
        keleres_rider: Code(2),
        drop_unofficial: false,
        blanked: false,
        hooked: 0,
        research: Some((ti4_engine_core_tests::inv::ALL_SOURCES, Box::leak(Box::default()))),
        fleet_pool: Code(3),
    }
}

fn close() -> Close {
    Close::new(facts(), Agenda::ChecksAndBalances, None, Some(Outcome::For), &[(Seat(1), Outcome::For)], &[(Outcome::For, 3)], &[Outcome::For, Outcome::Against])
}

fn rig(tag: PredictionTag, scenario: Scenario) -> Rig<Close> {
    let mut state = scenario.state();
    state.agenda_mut().predictions.insert(Seat(1), Prediction::Tagged(Outcome::For, tag));
    Rig::from_parts(state, kernel_all(), close())
}

fn state_of(r: &Rig<Close>) -> &CoreState {
    &r.state
}

#[test]
fn a_warfare_rider_places_a_dreadnought_in_the_one_system_with_a_ship_and_asks_among_several() {
    covers!("rules::agenda::close::Close");
    let (a, b) = two_planets();
    let (sa, sb) = (system_of(a), system_of(b));
    let base = || Scenario::lobby(3).map_from(&[sa, sb]).unit(sa, None, Unit::CruiserI, 1, 1);
    let mut r = rig(PredictionTag::WarRider, base());
    r.run();
    let ships = |r: &Rig<Close>, s: System| state_of(&r).board().sys(s).units.as_slice().iter().filter(|p| p.kind() == Unit::DreadnoughtI && p.owner() == Seat(1)).count();
    assert_eq!(ships(&r, sa), 1, "one system holds a ship of the seat: the reward goes there without a question");
    let mut r = rig(PredictionTag::WarRider, base().unit(sb, None, Unit::CruiserI, 1, 1));
    r.run();
    let choice = r.asked().clone();
    assert_eq!((choice.actor, choice.ask), (Seat(1), Ask::Agenda(AAsk::RiderTarget { rider: Rider::Warfare })));
    r.pick_opt(Opt::Agenda(AOpt::Spot { system: sb, planet: None }));
    assert_eq!((ships(&r, sa), ships(&r, sb)), (0, 1));
}

#[test]
fn a_diplomacy_rider_takes_a_fleet_token_from_every_other_seat_who_has_one() {
    covers!("rules::agenda::close::Close");
    let (a, _) = two_planets();
    let sa = system_of(a);
    let mut r = rig(PredictionTag::DiploRider, Scenario::lobby(3).map_from(&[sa]).control(a, 1).tokens(0, 0, 2, 0).tokens(2, 0, 2, 0));
    let before: Vec<i16> = (0..3).map(|i| state_of(&r).players().by_seat(Seat(i)).unwrap().econ.fleet_tokens).collect();
    r.run();
    let after: Vec<i16> = (0..3).map(|i| state_of(&r).players().by_seat(Seat(i)).unwrap().econ.fleet_tokens).collect();
    assert_eq!((after[1], after[0], after[2]), (before[1], before[0] - 1, before[2] - 1), "the others pay, the rider's seat does not");
    assert!(state_of(&r).board().sys(sa).cmd.contains(Seat(0)) && state_of(&r).board().sys(sa).cmd.contains(Seat(2)));
}
