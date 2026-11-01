//! Lane E2: the paid research whose bill takes the prerequisite away (`strategy_cards::paid_research`: `resolve_research` is false after the payment,
//! `*state = before`, and with no faction waiver to ask about the research ends with nothing gained). First gap of two games of the native sweep
//! (`SpaceDockII`, `DacxiveAnimators`): the core window ended `failed`, which the strategic action reports as a counted bail.

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::generated::{Planet, Source, TechType};
use ti4_engine_core::kernel::hooks::{ResearchWaiver, Waiver};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;
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

/// Answer the payment questions with their first option until the window asks something else.
fn pay_the_bill(rig: &mut Rig<Research>) {
    for _ in 0..16 {
        if rig.is_done() || matches!(rig.asked().ask, Ask::Strategy(_)) {
            return;
        }
        rig.pick(0);
    }
    panic!("the bill is never paid\n{}", rig.transcript());
}

/// A module offering a waiver for Gravity Drive.
fn walled(_: &CoreState, _: &Kernel, _: Seat, t: Technology) -> Option<ResearchWaiver> {
    (t == Technology::GravityDrive).then(|| ResearchWaiver { waiver: Waiver { module: Faction::TheVuilraithCabal, code: 0 }, payments: Default::default() })
}

/// The same module, but its waiver opens the technology only when the specialty planet that covered the prerequisite is exhausted (the bill took it).
fn walled_once_the_planet_is_spent(s: &CoreState, k: &Kernel, who: Seat, t: Technology) -> Option<ResearchWaiver> {
    walled(s, k, who, t).filter(|_| ti4_engine_core::query::board::exhausted(s, blue_planet()))
}

#[test]
fn a_technology_only_a_faction_waiver_opens_asks_the_waiver_after_the_bill_and_a_decline_gives_the_bill_back() {
    covers!("rules::production::strategy_cards::Research");
    // (Lane G5 asks the waiver question; before it the run was the string's, and this test pinned `failed`.)
    let (s, mut k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7).goods(0, 4).build();
    k.hooks.strategy.research_waiver_offer.push(walled);
    let mut rig = Rig::from_parts(s, k, Research::new(Seat(0), false, 4, ALL_SOURCES));
    rig.run();
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(Technology::GravityDrive))), "a waivable technology is offered");
    rig.pick_opt(Opt::Strategy(SOpt::Research(Technology::GravityDrive)));
    pay_the_bill(&mut rig);
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiver { .. })), "the waiver is asked: {:?}", rig.asked().ask);
    assert!(!rig.driver.window.failed, "the window carries the question");
    rig.pick_opt(Opt::Decline);
    // The technology was a waiver-only one BEFORE the bill: the question about the technologies comes again, without it.
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { decline: true, .. })), "{:?}", rig.asked().ask);
    assert!(!rig.opts().contains(&Opt::Strategy(SOpt::Research(Technology::GravityDrive))));
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the bill is back");
}

/// The join of lanes E2 and G5 (integration 14): the bill takes the specialty planet that covered the prerequisite, and a waiver opens the technology only
/// now. The string asks the waiver (`resolve_research`); declining it is `*state = before; if !waiver_required { return Ok(false) }`: `waiver_required` was
/// read BEFORE the bill (false), so the research ENDS with nothing gained. G5's window asked the technology again (it took every decline for a required waiver).
#[test]
fn a_waiver_that_opens_the_technology_only_after_the_bill_took_the_prerequisite_ends_the_research_when_declined() {
    covers!("rules::production::strategy_cards::Research");
    let blue = blue_planet();
    let bill = i64::from(blue.data().resources);
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system_of(blue), plain_systems()[0]]).control(blue, 0);
    let (s, mut k) = sc.build();
    k.hooks.strategy.research_waiver_offer.push(walled_once_the_planet_is_spent);
    let mut rig = Rig::from_parts(s, k, Research::new(Seat(0), false, bill, ALL_SOURCES));
    rig.run();
    assert!(rig.opts().contains(&Opt::Strategy(SOpt::Research(Technology::GravityDrive))), "the ready planet covers the prerequisite");
    rig.pick_opt(Opt::Strategy(SOpt::Research(Technology::GravityDrive)));
    pay_the_bill(&mut rig);
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiver { .. })), "the bill spent the planet: the waiver is asked: {:?}", rig.asked().ask);
    rig.pick_opt(Opt::Decline);
    assert!(rig.is_done(), "the research ends: {:?}", rig.asked().ask);
    assert!(!rig.driver.window.failed && rig.driver.window.researched.as_slice().is_empty());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the position from before the bill is back");
}

/// Lane N, found by a sweep game (`--factions sol --players 4 --first-seed 18`, first gap `StrategyAbility` of a Technology secondary): War Machine's
/// bonus (5, for the activation it was played in) is part of `production::available`, which counted it in the pool that decides whether a research is
/// offered; the bill then had no option and the window ended `failed` (a counted bail). The string's gate is `payment::affordable` (planets and trade
/// goods): a seat whose only pool is the bonus is not offered the research.
#[test]
fn war_machines_bonus_is_no_way_to_pay_a_research() {
    covers!("rules::production::strategy_cards::Research");
    use ti4_engine_core::state::RoundKind;
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[plain_systems()[0], plain_systems()[1]]);
    let mut rig = Rig::new(sc, Research::new(Seat(0), false, 4, ALL_SOURCES));
    let seq = rig.state.meta().activation_seq;
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, seq);
    rig.run();
    assert!(rig.is_done(), "no technology is offered: {:?}", rig.asked().ask);
    assert!(!rig.driver.window.failed && rig.driver.window.researched.as_slice().is_empty());
    // The control: four trade goods pay it (the research is offered), with the bonus standing or not.
    for marked in [false, true] {
        let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[plain_systems()[0], plain_systems()[1]]);
        let mut rig = Rig::new(sc, Research::new(Seat(0), false, 4, ALL_SOURCES));
        rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 4;
        if marked {
            let seq = rig.state.meta().activation_seq;
            rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, seq);
        }
        rig.run();
        assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { decline: true, .. })), "marked {marked}: {:?}", rig.asked().ask);
    }
}
