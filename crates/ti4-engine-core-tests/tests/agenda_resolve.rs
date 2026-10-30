//! D4-2a: `rules::agenda::effects::Resolve` for the effects that ask nothing: the point cards, the repeals, Mutiny, Holy Planet of Ixth,
//! the standing cards and the ones that defer.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::agenda::effects::{Done, Resolve};
use ti4_engine_core::state::Outcome;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, planets_with_influence};

fn vp(rig: &Rig<Resolve>, seat: u8) -> i16 {
    rig.state.players().by_seat(Seat(seat)).unwrap().econ.victory_points
}

fn resolve(agenda: Agenda, outcome: Outcome, voters: &[Seat], sc: Scenario, laws: &[Agenda]) -> Rig<Resolve> {
    let mut s = sc.state();
    for l in laws {
        s.agenda_mut().laws.insert(*l, Outcome::For);
    }
    let window = Resolve::of(agenda, outcome, voters).expect("an effect of the core");
    Rig::from_parts(s, kernel_all(), window)
}

fn table() -> Scenario {
    Scenario::lobby(3)
}

#[test]
fn the_elected_player_of_a_point_card_gains_one_victory_point() {
    covers!("rules::agenda::effects::Resolve");
    for agenda in [Agenda::PoliticalCensure, Agenda::ShardOfTheThrone, Agenda::TheCrownOfEmphidia] {
        let mut rig = resolve(agenda, Outcome::Seat(Seat(1)), &[], table(), &[]);
        rig.run();
        assert!(rig.is_done(), "{agenda:?}");
        assert_eq!((vp(&rig, 0), vp(&rig, 1)), (0, 1), "{agenda:?}");
        assert_eq!(rig.driver.window.done(), Done::Resolved);
    }
}

#[test]
fn judicial_abolishment_repeals_the_elected_law_and_a_missing_one_is_reported() {
    covers!("rules::agenda::effects::Resolve");
    let mut rig = resolve(Agenda::JudicialAbolishment, Outcome::Law(Agenda::MinisterOfWar), &[], table(), &[Agenda::MinisterOfWar, Agenda::MinisterOfPeace]);
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.window.repealed());
    assert!(!rig.state.agenda().laws.contains_key(&Agenda::MinisterOfWar));
    assert!(rig.state.agenda().laws.contains_key(&Agenda::MinisterOfPeace));
    let mut absent = resolve(Agenda::JudicialAbolishment, Outcome::Law(Agenda::MinisterOfWar), &[], table(), &[Agenda::MinisterOfPeace]);
    absent.run();
    assert!(!absent.driver.window.repealed());
    assert_eq!(absent.state.agenda().laws.len(), 1);
    ti4_engine_core_tests::assert_snapshot("agenda_resolve/judicial", &rig.snapshot());
}

#[test]
fn miscount_disclosed_repeals_but_defers_the_rest() {
    covers!("rules::agenda::effects::Resolve");
    let mut rig = resolve(Agenda::MiscountDisclosed, Outcome::Law(Agenda::MinisterOfWar), &[], table(), &[Agenda::MinisterOfWar]);
    rig.run();
    assert!(rig.driver.window.repealed());
    assert_eq!(rig.driver.window.done(), Done::Deferred);
}

#[test]
fn new_constitution_repeals_every_law_on_for_and_does_nothing_on_against() {
    covers!("rules::agenda::effects::Resolve");
    let laws = [Agenda::MinisterOfWar, Agenda::MinisterOfPeace];
    let mut rig = resolve(Agenda::NewConstitution, Outcome::For, &[], table(), &laws);
    rig.run();
    assert!(rig.state.agenda().laws.is_empty());
    let mut against = resolve(Agenda::NewConstitution, Outcome::Against, &[], table(), &laws);
    against.run();
    assert_eq!(against.state.agenda().laws.len(), 2);
}

#[test]
fn mutiny_pays_each_for_voter_a_point_or_takes_one_from_each_for_voter_on_against() {
    covers!("rules::agenda::effects::Resolve");
    let mut rig = resolve(Agenda::Mutiny, Outcome::For, &[Seat(0), Seat(2)], table(), &[]);
    rig.run();
    assert_eq!((vp(&rig, 0), vp(&rig, 1), vp(&rig, 2)), (1, 0, 1));
    let mut sc = table();
    sc = sc.round(1);
    let mut against = resolve(Agenda::Mutiny, Outcome::Against, &[Seat(0), Seat(2)], sc, &[]);
    for seat in [0u8, 2] {
        against.state.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.victory_points = 3;
    }
    against.run();
    assert_eq!((vp(&against, 0), vp(&against, 1), vp(&against, 2)), (2, 0, 2));
}

#[test]
fn holy_planet_of_ixth_pays_the_controller_of_the_elected_planet() {
    covers!("rules::agenda::effects::Resolve");
    let planet = planets_with_influence(&[1])[0];
    let sys = ti4_engine_core_tests::scenario::system_of(planet);
    let sc = table().map_from(&[sys]).control(planet, 2);
    let mut rig = resolve(Agenda::HolyPlanetOfIxth, Outcome::Planet(planet), &[], sc, &[]);
    rig.run();
    assert_eq!((vp(&rig, 0), vp(&rig, 2)), (0, 1));
}

#[test]
fn standing_cards_do_nothing_and_the_deferring_ones_say_so() {
    covers!("rules::agenda::effects::Resolve");
    let mut rig = resolve(Agenda::ProphecyOfIxth, Outcome::For, &[], table(), &[Agenda::ProphecyOfIxth]);
    let before = rig.state.clone();
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.done(), Done::Resolved);
    assert!(rig.state == before, "a standing card changes nothing when it passes");
    let mut deferred = resolve(Agenda::AntiIntellectualRevolution, Outcome::Against, &[], table(), &[]);
    deferred.run();
    assert_eq!(deferred.driver.window.done(), Done::Deferred);
}

#[test]
fn an_outcome_the_effect_does_not_elect_has_no_window() {
    covers!("rules::agenda::effects::Resolve");
    assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::For, &[]).is_none());
    assert!(Resolve::of(Agenda::HolyPlanetOfIxth, Outcome::For, &[]).is_none());
    let (_, planet) = first_planet();
    assert!(Resolve::of(Agenda::Mutiny, Outcome::Planet(planet), &[]).is_none());
    let repeal = Resolve::repeal(Agenda::MinisterOfWar);
    assert!(!repeal.rolls() && !repeal.draws() && !repeal.needs_map());
    let _ = SmallList::<Seat, 8>::new();
}
