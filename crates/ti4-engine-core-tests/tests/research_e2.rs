//! Lane E2: the paid research whose bill takes the prerequisite away (`strategy_cards::paid_research`: `resolve_research` is false after the payment,
//! `*state = before`, and with no faction waiver to ask about the research ends with nothing gained). First gap of two games of the native sweep
//! (`SpaceDockII`, `DacxiveAnimators`): the core window ended `failed`, which the strategic action reports as a counted bail.

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::generated::{Planet, Source, TechType};
use ti4_engine_core::rules::production::strategy_cards::Research;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::Policy;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn blue_planet() -> Planet {
    Planet::ALL.into_iter().find(|p| p.data().source == Source::Base && p.data().specialties == [TechType::Propulsion] && p.data().resources > 0).expect("a blue specialty planet that pays")
}

#[test]
fn a_bill_paid_with_the_specialty_planet_that_covered_the_prerequisite_puts_the_position_back_and_ends_the_research() {
    covers!("rules::production::strategy_cards::Research");
    let blue = blue_planet();
    let bill = i64::from(blue.data().resources);
    let sc = || Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system_of(blue), plain_systems()[0]]).control(blue, 0);
    // The secondary costs exactly what the blue planet is worth, and the planet is the one blue prerequisite of Gravity Drive.
    let mut rig = Rig::new(sc(), Research::new(Seat(0), false, bill, ALL_SOURCES));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { decline: true, .. })), "{:?}", rig.asked().ask);
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(Technology::GravityDrive))), "the list counts the ready specialty planet");
    rig.pick_opt(Opt::Strategy(SOpt::Research(Technology::GravityDrive)));
    rig.auto(Policy::First);
    assert!(rig.is_done());
    assert!(!rig.driver.window.failed, "the string ends the research itself: nothing is left to the string");
    assert!(rig.driver.window.researched.as_slice().is_empty());
    assert!(!rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::GravityDrive));
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the payment is undone: the position from before the bill is back");
}

#[test]
fn a_bill_paid_by_another_planet_leaves_the_specialty_ready_and_the_technology_lands() {
    covers!("rules::production::strategy_cards::Research");
    let blue = blue_planet();
    let other = Planet::ALL.into_iter().find(|p| p.data().system.is_some() && *p != blue && p.data().specialties.is_empty() && p.data().resources >= 1 && p.data().source == Source::Base).expect("a plain planet");
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system_of(blue), system_of(other)]).control(blue, 0).control(other, 0);
    let mut rig = Rig::new(sc, Research::new(Seat(0), false, i64::from(other.data().resources), ALL_SOURCES));
    rig.run();
    rig.pick_opt(Opt::Strategy(SOpt::Research(Technology::GravityDrive)));
    // Pay with the plain planet (the offered payment that is not the blue one).
    let mut guard = 0;
    while !rig.is_done() && guard < 8 {
        let pick = rig.opts().into_iter().position(|o| format!("{o:?}").contains(&format!("{other:?}"))).unwrap_or(0);
        rig.pick(u8::try_from(pick).unwrap_or(0).into());
        guard += 1;
    }
    assert!(rig.is_done() && !rig.driver.window.failed);
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::GravityDrive), "the bill left the prerequisite standing");
}

#[test]
fn a_technology_only_a_faction_waiver_opens_is_still_left_to_the_string_after_the_bill() {
    covers!("rules::production::strategy_cards::Research");
    use ti4_engine_core::kernel::hooks::{ResearchWaiver, Waiver};
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::state::CoreState;
    fn walled(_: &CoreState, _: &Kernel, _: Seat, t: Technology) -> Option<ResearchWaiver> {
        (t == Technology::GravityDrive).then(|| ResearchWaiver { waiver: Waiver { module: Faction::TheVuilraithCabal, code: 0 }, payments: Default::default() })
    }
    // (G5 asks the waiver question there; until it lands the run is the string's: a counted bail, not a rollback.)
    let (s, mut k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7).goods(0, 4).build();
    k.hooks.strategy.research_waiver_offer.push(walled);
    let mut rig = Rig::from_parts(s, k, Research::new(Seat(0), false, 4, ALL_SOURCES));
    rig.run();
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(Technology::GravityDrive))), "a waivable technology is offered");
    rig.pick_opt(Opt::Strategy(SOpt::Research(Technology::GravityDrive)));
    rig.auto(Policy::First);
    assert!(rig.is_done());
    assert!(rig.driver.window.failed, "the waiver question is not carried: the string performs the research");
}
