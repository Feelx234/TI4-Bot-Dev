//! D4 lane A3: Leadership and Construction beyond lane A1: the payment loop (several planets and trade goods for one token, the overpayment that is
//! carried to the next token), the structures a supply or a zone allows, a structure in space (Floating Factory), the Thunder's Edge Construction.
//! The printed text: Leadership "gain 3 command tokens ... spend any amount of influence to gain 1 command token for every 3 influence you spend";
//! Construction "place 1 PDS or 1 space dock on a planet you control; place 1 additional PDS on a planet you control".

use ti4_engine_core::choice::production::Opt as POpt;
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::generated::{Agenda, Attachment};
use ti4_engine_core::kernel::children::StructureFacts;
use ti4_engine_core::rules::production::strategy_cards::{CStage, Construction, LStage, Leadership};
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, system_of, systems_of};

fn strat(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

fn table(influence: &[u8], goods: i16) -> (Scenario, Vec<Planet>) {
    let planets = planets_with_influence(influence);
    let mut sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&systems_of(&planets)).goods(0, goods).tokens(0, 1, 1, 1);
    for p in &planets {
        sc = sc.control(*p, 0);
    }
    (sc, planets)
}

fn tokens(s: &CoreState) -> (i16, i16, i16) {
    let e = s.players().by_seat(Seat(0)).unwrap().econ;
    (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
}

fn exhausted(s: &CoreState, p: Planet) -> bool {
    s.board().planet(p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)
}

fn pay_planet(p: Planet) -> impl Fn(&Opt) -> bool {
    move |o| matches!(o, Opt::Production(POpt::Exhaust { planet, .. }) if *planet == p)
}

// ---- Leadership ----------------------------------------------------------------------------------------

#[test]
fn each_free_token_goes_to_the_pool_chosen_for_it() {
    let (sc, _) = table(&[1], 0);
    let mut rig = Rig::new(sc, Leadership::new(Seat(0), true));
    rig.run();
    assert_eq!(rig.driver.window.stage(), LStage::Gain);
    rig.pick_opt(strat(SOpt::Pool(Pool::Fleet)));
    rig.pick_opt(strat(SOpt::Pool(Pool::Fleet)));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    rig.pick_opt(strat(SOpt::Pool(Pool::Strategic)));
    assert_eq!(tokens(&rig.state), (1, 3, 2), "two to the fleet, one to the strategy pool");
    assert_eq!(rig.driver.window.stage(), LStage::Done, "one influence cannot buy a token: no offer");
}

#[test]
fn a_token_is_paid_with_several_planets_and_the_overpayment_is_carried_to_the_next() {
    // Three planets of 2 influence: 3 for the first token (2 + 2 = 4, one carried), 3 for the second (1 carried + 2).
    let (sc, ps) = table(&[2, 2, 2], 0);
    let mut rig = Rig::new(sc, Leadership::new(Seat(0), false));
    rig.run();
    assert_eq!(rig.driver.window.stage(), LStage::Pay, "the secondary pays at once");
    let AskPay { owed, kind } = pay_ask(&rig);
    assert_eq!((owed, kind), (3, Currency::Influence));
    rig.assert_options_sound();
    rig.pick_where(pay_planet(ps[0]));
    assert_eq!(pay_ask(&rig).owed, 1, "one still owed");
    rig.pick_where(pay_planet(ps[1]));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    rig.pick_opt(strat(SOpt::Pool(Pool::Tactic)));
    assert_eq!(tokens(&rig.state).0, 2);
    // The window is the secondary: one purchase only.
    assert!(rig.is_done() || matches!(rig.asked().ask, Ask::Strategy(SAsk::LeadershipBuy { .. })));
}

use ti4_engine_core::choice::production::{Ask as PAsk, Currency};

struct AskPay {
    owed: i64,
    kind: Currency,
}

fn pay_ask<W: ti4_engine_core::kernel::stage::Window>(rig: &Rig<W>) -> AskPay {
    match &rig.asked().ask {
        Ask::Production(PAsk::Pay(p)) => AskPay { owed: p.owed, kind: p.kind },
        other => panic!("not a payment: {other:?}"),
    }
}

#[test]
fn the_primary_offers_again_with_the_carried_influence_and_the_credit_pays_part_of_the_next_token() {
    let (sc, ps) = table(&[2, 2, 2], 0);
    let mut rig = Rig::new(sc, Leadership::new(Seat(0), true));
    rig.run().pick(0).pick(0).pick(0);
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::LeadershipBuy { owed: 3, more: false }));
    rig.pick_opt(strat(SOpt::Buy(true)));
    rig.pick_where(pay_planet(ps[0]));
    rig.pick_where(pay_planet(ps[1]));
    rig.pick(0); // the pool of the token bought
    let (a, b, c) = tokens(&rig.state);
    assert_eq!(a + b + c, 3 + 3 + 1, "three free tokens and one bought");
    // The carried influence shortens the next bill: 3 - 1 = 2 are owed.
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::LeadershipBuy { owed: 2, more: true }));
    rig.pick_opt(strat(SOpt::Buy(true)));
    // The last planet is the only way to pay: it is taken without a question, and the token's pool is asked.
    assert!(exhausted(&rig.state, ps[2]));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    rig.pick(1);
    assert_eq!(tokens(&rig.state).0 + tokens(&rig.state).1 + tokens(&rig.state).2, 3 + 3 + 2, "two tokens bought in all");
    assert!(rig.is_done(), "nothing left to pay with, no offer");
    assert!(ps.iter().all(|p| exhausted(&rig.state, *p)));
}

#[test]
fn trade_goods_pay_for_a_token_and_a_bill_met_with_goods_alone_needs_three_of_them() {
    let (sc, ps) = table(&[2], 1);
    let mut rig = Rig::new(sc, Leadership::new(Seat(0), false));
    rig.run();
    assert_eq!(pay_ask(&rig).owed, 3);
    assert!(rig.opts().iter().any(|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))));
    rig.pick_where(pay_planet(ps[0]));
    // One influence short: the single trade good is the only option and it is taken without a question.
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
    // Three trade goods and no planet also buy a token.
    let none = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).goods(0, 3).tokens(0, 1, 1, 1);
    let mut goods = Rig::new(none, Leadership::new(Seat(0), false));
    goods.run(); // each lone trade good is taken without a question
    assert_eq!(goods.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    assert_eq!(goods.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
    // Two trade goods: not enough, the window ends.
    let two = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).goods(0, 2);
    let mut short = Rig::new(two, Leadership::new(Seat(0), false));
    short.run();
    assert!(short.is_done());
}

#[test]
fn a_wrong_answer_to_leadership_is_refused_and_leaves_the_stage() {
    let (sc, _) = table(&[3], 0);
    let mut rig = Rig::new(sc, Leadership::new(Seat(0), true));
    rig.run();
    rig.expect_illegal(Pick::Opt(strat(SOpt::Buy(true))));
    rig.expect_illegal(Pick::Number(1));
    assert_eq!(rig.driver.window.stage(), LStage::Gain);
    assert_eq!(rig.driver.window.who, Seat(0));
}

// ---- Construction -----------------------------------------------------------------------------------------

fn facts() -> StructureFacts {
    StructureFacts { pds: Some((Unit::PDSI, false)), dock: Some((Unit::SpaceDockI, false)), source: Code(3) }
}

fn two_planets() -> (Scenario, Vec<Planet>) {
    let planets = planets_with_influence(&[1, 2]);
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&systems_of(&planets)).control(planets[0], 0).control(planets[1], 0);
    (sc, planets)
}

fn structure(pds: bool, planet: Planet) -> Opt {
    Opt::Strategy(SOpt::Structure { pds, system: system_of(planet), planet })
}

#[test]
fn a_structure_that_is_not_in_the_supply_any_more_is_not_offered() {
    let (sc, ps) = two_planets();
    // Three space docks stand elsewhere: the plastic is gone, a PDS is still on offer.
    let sc = sc
        .unit(System::Wellon, Some(Planet::Wellon), Unit::SpaceDockI, 0, 1)
        .unit(System::Quann, Some(Planet::Quann), Unit::SpaceDockI, 0, 1)
        .unit(System::Lodor, Some(Planet::Lodor), Unit::SpaceDockI, 0, 1);
    let mut rig = Rig::new(sc, Construction::new(Seat(0), false, facts()));
    rig.run();
    let opts = rig.opts();
    for p in &ps {
        assert!(!opts.contains(&structure(false, *p)), "no dock left in the box");
        assert!(opts.contains(&structure(true, *p)));
    }
}

#[test]
fn space_stations_demilitarized_planets_and_planets_of_others_are_not_places_for_a_structure() {
    let (sc, ps) = two_planets();
    let station = Planet::OluzStation;
    let sc = sc.control(station, 0).control(Planet::Quann, 1);
    let mut s = sc.clone().state();
    s.agenda_mut().laws.insert(Agenda::DemilitarizedZone, Outcome::Planet(ps[1]));
    let mut rig = Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), Construction::new(Seat(0), false, facts()));
    rig.run();
    let opts = rig.opts();
    assert!(opts.contains(&structure(true, ps[0])) && opts.contains(&structure(false, ps[0])));
    assert!(opts.iter().all(|o| !matches!(o, Opt::Strategy(SOpt::Structure { planet, .. }) if *planet == ps[1] || *planet == station || *planet == Planet::Quann)));
    // The demilitarization by attachment (rules version 34 and later) does the same.
    let (sc2, ps2) = two_planets();
    let mut s2 = sc2.state();
    s2.board_mut().planet_mut(ps2[0]).attach.push(Attachment::DemilitarizedZone);
    let mut rig2 = Rig::from_parts(s2, ti4_engine_core_tests::kernel_all(), Construction::new(Seat(0), false, facts()));
    rig2.run();
    assert!(rig2.opts().iter().all(|o| !matches!(o, Opt::Strategy(SOpt::Structure { planet, .. }) if *planet == ps2[0])));
    assert!(rig2.opts().contains(&structure(true, ps2[1])));
}

#[test]
fn a_structure_placed_in_space_goes_to_the_space_area_of_the_system() {
    let (sc, ps) = two_planets();
    let in_space = StructureFacts { pds: Some((Unit::PDSI, false)), dock: Some((Unit::FloatingFactoryI, true)), source: Code(3) };
    let mut rig = Rig::new(sc, Construction::new(Seat(0), false, in_space));
    rig.run();
    // A space-only structure is offered on every planet regardless of the planet's limit, and lands in space.
    assert!(rig.opts().contains(&structure(false, ps[0])));
    rig.pick_opt(structure(false, ps[0]));
    assert!(rig.is_done());
    let at = system_of(ps[0]);
    assert_eq!(rig.state.board().sys(at).units.as_slice().iter().filter(|u| u.kind() == Unit::FloatingFactoryI).count(), 1);
    assert!(rig.state.board().planet(ps[0]).units.is_empty(), "not on the planet");
    assert!(rig.state.board().sys(at).flags.has(ti4_engine_core::state::SysFlags::TOUCHED));
    // The in-space kind is not held back by the dock limit of the planet.
    let (sc, ps) = two_planets();
    let sc = sc.unit(system_of(ps[0]), Some(ps[0]), Unit::SpaceDockI, 0, 1);
    let mut again = Rig::new(sc, Construction::new(Seat(0), false, in_space));
    again.run();
    assert!(again.opts().contains(&structure(false, ps[0])));
}

#[test]
fn a_kind_the_seat_does_not_place_is_not_offered() {
    let (sc, ps) = two_planets();
    let only_pds = StructureFacts { pds: Some((Unit::PDSI, false)), dock: None, source: Code(3) };
    let mut rig = Rig::new(sc.clone(), Construction::new(Seat(0), false, only_pds));
    rig.run();
    assert!(rig.opts().iter().all(|o| !matches!(o, Opt::Strategy(SOpt::Structure { pds: false, .. }))));
    let neither = StructureFacts { pds: None, dock: None, source: Code(3) };
    let mut none = Rig::new(sc, Construction::new(Seat(0), true, neither));
    none.run();
    assert!(none.is_done(), "nothing to place: nothing asked");
    let _ = ps;
}

#[test]
fn the_primary_walks_through_its_stages_and_a_wrong_stage_answer_is_refused() {
    let (sc, ps) = two_planets();
    let mut rig = Rig::new(sc, Construction::new(Seat(0), true, facts()));
    assert_eq!(rig.driver.window.stage(), CStage::Start);
    rig.run();
    assert_eq!(rig.driver.window.stage(), CStage::First);
    rig.expect_illegal(Pick::Opt(Opt::Strategy(SOpt::Produce(system_of(ps[0])))));
    rig.pick_opt(structure(true, ps[0]));
    assert_eq!(rig.driver.window.stage(), CStage::Second);
    // The same planet may take a second PDS (two per planet).
    assert!(rig.opts().contains(&structure(true, ps[0])));
    rig.pick_opt(structure(true, ps[0]));
    assert_eq!(rig.driver.window.stage(), CStage::Done);
    assert_eq!(rig.state.board().planet(ps[0]).units.as_slice().iter().filter(|u| u.kind() == Unit::PDSI).count(), 2);
}

#[test]
fn a_planet_that_took_the_first_pds_may_not_take_a_third_and_the_second_step_is_skipped_when_nothing_fits() {
    let (sc, ps) = two_planets();
    let at = system_of(ps[0]);
    let sc = sc.unit(at, Some(ps[0]), Unit::PDSI, 0, 1).control(ps[1], 1);
    // Only planet 0 is ours now (planet 1 went to the rival): a PDS there, then one more is allowed, then no third.
    let mut s = sc.state();
    s.board_mut().planet_mut(ps[1]).control = Some(Seat(1));
    let mut rig = Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), Construction::new(Seat(0), true, facts()));
    rig.run().pick_opt(structure(true, ps[0]));
    assert!(rig.is_done(), "the planet holds two PDS: the second structure has no place and is skipped");
}

#[test]
fn the_thunders_edge_primary_asks_for_a_structure_or_a_production_first_and_neither_structure_is_limited_to_a_pds() {
    let (sc, ps) = two_planets();
    let systems = [system_of(ps[0])];
    let source = Code(9);
    let mut rig = Rig::new(sc.clone(), Construction::new_te(Seat(0), facts(), source, &systems));
    rig.run();
    assert_eq!(rig.driver.window.stage(), CStage::Ability);
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ConstructionAbility)));
    assert_eq!(rig.opts(), vec![strat(SOpt::Build), strat(SOpt::Produce(systems[0]))]);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.expect_illegal(Pick::Opt(structure(true, ps[0])));
    rig.pick_opt(strat(SOpt::Build));
    assert_eq!(rig.driver.window.stage(), CStage::First);
    rig.pick_opt(structure(false, ps[0]));
    // Unlike the printed base-game card, the second structure may be a dock too.
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::PlaceStructure { step: 2, only_pds: false })));
    assert!(rig.opts().contains(&structure(false, ps[1])));
    // Production instead: the window ends and says so.
    let mut produce = Rig::new(sc, Construction::new_te(Seat(0), facts(), source, &systems));
    produce.run().pick_opt(strat(SOpt::Produce(systems[0])));
    assert!(produce.is_done());
    assert!(produce.driver.window.produces);
    assert!(produce.state.board().planet(ps[0]).units.is_empty());
}
