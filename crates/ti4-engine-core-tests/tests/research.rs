//! D4-2a: `rules::production::strategy_cards::Research` (the Technology strategy card: one free research for the primary, a paid one after).

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::generated::{Planet, Source, TechType};
use ti4_engine_core::query::board;
use ti4_engine_core::rules::production::strategy_cards::Research;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn research(primary: bool, cost: i64) -> Research {
    Research::new(Seat(0), primary, cost, ALL_SOURCES)
}

fn base() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7)
}

fn blue_planet() -> Planet {
    Planet::ALL.into_iter().find(|p| p.data().source == Source::Base && p.data().specialties == [TechType::Propulsion]).expect("a blue specialty planet")
}

fn rt(t: Technology) -> Opt {
    Opt::Strategy(SOpt::Research(t))
}

#[test]
fn the_primary_researches_one_technology_for_free_among_those_whose_prerequisites_it_meets() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = Rig::new(base(), research(true, 6));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { cost: 0, owned: 0, decline: false })), "{:?}", rig.asked().ask);
    let opts = rig.opts();
    assert!(opts.contains(&rt(Technology::AntimassDeflectors)), "no prerequisite: open to a seat without technologies");
    assert!(!opts.contains(&rt(Technology::GravityDrive)), "needs a blue prerequisite the seat lacks");
    assert!(!opts.contains(&Opt::Decline), "the free research cannot be declined");
    rig.assert_options_sound();
    rig.pick_opt(rt(Technology::AntimassDeflectors));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::AntimassDeflectors));
    assert_eq!(rig.driver.window.researched.as_slice(), &[Technology::AntimassDeflectors]);
    assert!(!rig.driver.window.failed);
    rig.auto(ti4_engine_core_tests::rig::Policy::Last);
    assert!(rig.is_done());
}

#[test]
fn a_specialty_planet_covers_a_prerequisite_and_is_exhausted_by_the_research_unless_a_card_covers_it() {
    covers!("rules::production::strategy_cards::Research");
    let blue = blue_planet();
    let sc = || Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system_of(blue), plain_systems()[0]]);
    let mut rig = Rig::new(sc().control(blue, 0), research(true, 6));
    rig.run();
    assert!(rig.opts().contains(&rt(Technology::GravityDrive)), "one blue specialty is one blue prerequisite");
    rig.pick_opt(rt(Technology::GravityDrive));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::GravityDrive));
    assert!(board::exhausted(&rig.state, blue), "the planet that supplied the prerequisite is exhausted");
    let mut covered = Rig::new(sc().control(blue, 0).tech(0, Technology::AntimassDeflectors), research(true, 6));
    covered.run();
    covered.pick_opt(rt(Technology::GravityDrive));
    assert!(!covered.state.board().planet(blue).flags.has(PlanetFlags::EXHAUSTED), "a blue technology card already covers it: the planet stays ready");
}

#[test]
fn a_secondary_without_the_resources_to_pay_asks_nothing_and_changes_nothing() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = Rig::new(base(), research(false, 4));
    rig.run();
    assert!(rig.is_done() || rig.opts().last() == Some(&Opt::Decline), "either nothing is affordable or it can be declined");
    if !rig.is_done() {
        rig.decline();
    }
    assert!(rig.is_done());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn research_smoke() {
    let blue = blue_planet();
    smoke("research/primary_with_a_blue_planet", || Rig::new(Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system_of(blue), plain_systems()[0]]).control(blue, 0), research(true, 4)));
    smoke_quiet(|| Rig::new(base(), research(false, 0)));
}
