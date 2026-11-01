//! Lane H4: Doctor Sucaban (the Jol-Nar agent) and Ssruu's copy of it in the paid research of the Technology card
//! (`Research`'s `Offer` and `Trade` stages; the string's `strategy_cards::{doctor_sucaban, sucaban_sources, trade_infantry_for_research}`).
//! The printed text: "Doctor Sucaban: when a player would spend resources to research a technology, you may exhaust this card to allow that
//! player to remove infantry from the board to reduce the cost by 1 for each".

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
use ti4_engine_core::choice::{Origin, Subtype};
use ti4_engine_core::generated::{Leader, UnitBaseType};
use ti4_engine_core::rules::production::strategy_cards::Research;
use ti4_engine_core::state::{LeaderStatus, Piece};
use ti4_engine_core_tests::delta::delta;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::system_of;

fn strat(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

fn standing(system: System, planet: Option<Planet>) -> Opt {
    strat(SOpt::Infantry { system, planet })
}

fn set_leader(s: &mut CoreState, seat: u8, leader: Leader, status: LeaderStatus) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().leaders.status.insert(leader, status);
}

fn infantry_on(s: &CoreState, p: Planet) -> usize {
    s.board().planet(p).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind().data().base_type == UnitBaseType::Infantry).count()
}

fn status(s: &CoreState, seat: u8, leader: Leader) -> Option<LeaderStatus> {
    s.players().by_seat(Seat(seat)).and_then(|p| p.leaders.status.get(&leader).copied())
}

/// Seat 0 researches (a secondary for `cost`) with `infantry` infantry on Starpoint (3 resources, controlled, ready) and Zohbat (3 resources,
/// controlled); seat 1 is Jol-Nar with a readied Doctor Sucaban; seat 2 plays `third`.
fn table(third: Faction, cost: i64, infantry: usize) -> Rig<Research> {
    let mut sc = Scenario::new(&[Faction::TheArborec, Faction::TheUniversitiesOfJolNar, third])
        .control(Planet::Starpoint, 0)
        .map_from(&[system_of(Planet::Starpoint)])
        .unit(system_of(Planet::Starpoint), Some(Planet::Starpoint), Unit::InfantryI, 0, infantry.max(1));
    if infantry == 0 {
        sc = Scenario::new(&[Faction::TheArborec, Faction::TheUniversitiesOfJolNar, third]).control(Planet::Starpoint, 0).map_from(&[system_of(Planet::Starpoint)]);
    }
    let (mut s, k) = sc.build();
    set_leader(&mut s, 1, Leader::DoctorSucaban, LeaderStatus::Readied);
    Rig::from_parts(s, k, Research::new(Seat(0), false, cost, ti4_engine_core_tests::inv::ALL_SOURCES))
}

#[test]
fn the_holder_of_the_agent_is_offered_the_trade_before_the_research_can_be_afforded() {
    covers!("rules::production::strategy_cards::Research");
    // Four resources are owed and Starpoint pays three: without the agent the seat cannot research.
    let mut rig = table(Faction::TheXxchaKingdom, 4, 2);
    rig.run();
    let asked = rig.asked();
    assert_eq!(asked.actor, Seat(1), "the holder of the agent answers, not the researcher");
    assert_eq!(asked.ask, Ask::Strategy(SAsk::SucabanOffer { researcher: Seat(0), source: Seat(1), copied: false }));
    assert_eq!(asked.ctx.origin, Origin::Content(Content::Leader(Leader::DoctorSucaban)));
    assert_eq!(asked.ctx.subtype, Subtype::Strategy(SSub::SucabanExhaust));
    assert_eq!(rig.opts(), vec![strat(SOpt::Exhaust), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(strat(SOpt::Exhaust));
    assert_eq!(status(&rig.state, 1, Leader::DoctorSucaban), Some(LeaderStatus::Exhausted), "the agent is exhausted by the accepted offer");
    assert_eq!(rig.asked().actor, Seat(0), "the researcher chooses the infantry");
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::SucabanTrade));
    assert_eq!(rig.asked().ctx.origin, Origin::Content(Content::Leader(Leader::DoctorSucaban)));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Strategy(SSub::SucabanRemoveInfantry));
    assert_eq!(rig.opts(), vec![standing(system_of(Planet::Starpoint), Some(Planet::Starpoint)), Opt::Decline]);
    rig.pick(0);
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 1, "one infantry left the board");
    // The second infantry is offered too; the researcher stops here: the bill is 3.
    rig.decline();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { cost: 3, decline: true, .. })), "{:?}", rig.asked().ask);
    rig.pick_opt(strat(SOpt::Research(Technology::AntimassDeflectors)));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::AntimassDeflectors));
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 1);
    assert_eq!(status(&rig.state, 1, Leader::DoctorSucaban), Some(LeaderStatus::Exhausted));
}

#[test]
fn every_infantry_traded_takes_one_off_the_bill_and_the_trade_ends_with_the_bill() {
    covers!("rules::production::strategy_cards::Research");
    // Two resources owed, five infantry: after the second the bill is paid and the trade is not asked again.
    let mut rig = table(Faction::TheXxchaKingdom, 2, 5);
    rig.run().pick_opt(strat(SOpt::Exhaust));
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::SucabanTrade)), "a bill of 1 is still open");
    rig.pick(0);
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 3);
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { cost: 0, decline: true, .. })), "{:?}", rig.asked().ask);
    rig.pick_opt(strat(SOpt::Research(Technology::AntimassDeflectors)));
    assert!(rig.is_done(), "a research that costs nothing pays nothing: {}", rig.transcript());
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::AntimassDeflectors));
}

#[test]
fn a_declined_offer_changes_nothing_and_the_unaffordable_research_ends() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = table(Faction::TheXxchaKingdom, 4, 2);
    rig.run().decline();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    assert_eq!(status(&rig.state, 1, Leader::DoctorSucaban), Some(LeaderStatus::Readied));
}

#[test]
fn nothing_is_offered_without_a_readied_agent_infantry_to_trade_or_a_bill() {
    covers!("rules::production::strategy_cards::Research");
    // The agent is exhausted.
    let mut rig = table(Faction::TheXxchaKingdom, 4, 2);
    set_leader(&mut rig.state, 1, Leader::DoctorSucaban, LeaderStatus::Exhausted);
    let mut rig = Rig::from_parts(rig.state.clone(), rig.kernel.clone(), Research::new(Seat(0), false, 4, ti4_engine_core_tests::inv::ALL_SOURCES));
    rig.run();
    assert!(rig.is_done(), "an exhausted agent asks nothing and the research is not affordable");
    // No infantry on the board.
    let mut none = table(Faction::TheXxchaKingdom, 4, 0);
    none.run();
    assert!(none.is_done());
    // The card is another seat's Ssruu without a Jol-Nar agent to copy: nothing either.
    let mut free = table(Faction::TheXxchaKingdom, 0, 2);
    free.run();
    assert!(matches!(free.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { cost: 0, .. })), "a bill of nothing is not traded against: {:?}", free.asked().ask);
}

#[test]
fn the_agent_can_be_declined_again_by_the_next_source_and_the_order_follows_initiative_from_the_active_seat() {
    covers!("rules::production::strategy_cards::Research");
    // Seat 0 holds a readied Ssruu; seat 2 holds the agent (Jol-Nar), readied: two rights, the agent of seat 2 and seat 0's copy.
    let build = |active: u8| {
        let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheXxchaKingdom, Faction::TheUniversitiesOfJolNar])
            .control(Planet::Starpoint, 0)
            .map_from(&[system_of(Planet::Starpoint)])
            .unit(system_of(Planet::Starpoint), Some(Planet::Starpoint), Unit::InfantryI, 0, 2)
            .active(Some(active))
            .build();
        set_leader(&mut s, 0, Leader::Ssruu, LeaderStatus::Readied);
        set_leader(&mut s, 2, Leader::DoctorSucaban, LeaderStatus::Readied);
        Rig::from_parts(s, k, Research::new(Seat(0), false, 4, ti4_engine_core_tests::inv::ALL_SOURCES))
    };
    // From seat 0 (the researcher is the active seat): the copy of seat 0 comes before the agent of seat 2.
    let mut rig = build(0);
    rig.run();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(0), Ask::Strategy(SAsk::SucabanOffer { researcher: Seat(0), source: Seat(2), copied: true })));
    assert_eq!(rig.asked().ctx.origin, Origin::Content(Content::Leader(Leader::Ssruu)));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Strategy(SSub::SucabanBorrowedExhaust));
    rig.decline();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(2), Ask::Strategy(SAsk::SucabanOffer { researcher: Seat(0), source: Seat(2), copied: false })), "a declined offer does not close the next one");
    // From seat 1 (the order is 1, 2, 0): the agent of seat 2 comes first.
    let mut rig = build(1);
    rig.run();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(2), Ask::Strategy(SAsk::SucabanOffer { researcher: Seat(0), source: Seat(2), copied: false })));
    rig.decline();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(0), Ask::Strategy(SAsk::SucabanOffer { researcher: Seat(0), source: Seat(2), copied: true })));
}

#[test]
fn a_copy_through_ssruu_is_undone_when_no_technology_lands_and_kept_when_one_does() {
    covers!("rules::production::strategy_cards::Research");
    let build = || {
        let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheXxchaKingdom, Faction::TheUniversitiesOfJolNar])
            .control(Planet::Starpoint, 0)
            .map_from(&[system_of(Planet::Starpoint)])
            .unit(system_of(Planet::Starpoint), Some(Planet::Starpoint), Unit::InfantryI, 0, 2)
            .build();
        set_leader(&mut s, 0, Leader::Ssruu, LeaderStatus::Readied);
        // The agent itself is exhausted: a copy may still use it.
        set_leader(&mut s, 2, Leader::DoctorSucaban, LeaderStatus::Exhausted);
        Rig::from_parts(s, k, Research::new(Seat(0), false, 4, ti4_engine_core_tests::inv::ALL_SOURCES))
    };
    // The technology is declined after the trade: the exhausted Ssruu and the traded infantry come back.
    let mut rig = build();
    rig.run().pick_opt(strat(SOpt::Exhaust));
    assert_eq!(status(&rig.state, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted));
    rig.pick(0);
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 1);
    rig.decline();
    rig.decline();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "no technology landed: the copy is undone as a whole");
    assert_eq!(status(&rig.state, 0, Leader::Ssruu), Some(LeaderStatus::Readied));
    // A bill the infantry did not make affordable (nothing traded) ends the same way.
    let mut rig = build();
    rig.run().pick_opt(strat(SOpt::Exhaust));
    rig.decline();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "an unaffordable bill after a copy: undone");
    // A technology lands: Ssruu stays exhausted and the infantry stay off the board.
    let mut rig = build();
    rig.run().pick_opt(strat(SOpt::Exhaust));
    rig.pick(0);
    rig.decline();
    rig.pick_opt(strat(SOpt::Research(Technology::AntimassDeflectors)));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::AntimassDeflectors));
    assert_eq!(status(&rig.state, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted));
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 1);
    // The agent that the copy used stays as it was (exhausted before, exhausted after).
    assert_eq!(status(&rig.state, 2, Leader::DoctorSucaban), Some(LeaderStatus::Exhausted));
}

#[test]
fn the_places_are_listed_in_the_order_of_the_string_the_space_area_before_the_planets_of_a_system() {
    covers!("rules::production::strategy_cards::Research");
    let system = system_of(Planet::Starpoint);
    let mut rig = table(Faction::TheXxchaKingdom, 4, 1);
    // An infantry in the space area of the same system (a carried ground force): the string lists the space area first.
    rig.state.board_mut().sys_mut(system).units.push(Piece::new(Unit::InfantryI, Seat(0)));
    let mut rig = Rig::from_parts(rig.state.clone(), rig.kernel.clone(), Research::new(Seat(0), false, 4, ti4_engine_core_tests::inv::ALL_SOURCES));
    rig.run().pick_opt(strat(SOpt::Exhaust));
    assert_eq!(rig.opts(), vec![standing(system, None), standing(system, Some(Planet::Starpoint)), Opt::Decline]);
    rig.pick(0);
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(0)).count(), 0, "the space infantry left");
    assert_eq!(infantry_on(&rig.state, Planet::Starpoint), 1);
}

#[test]
fn a_ssruu_with_no_agent_to_copy_asks_nothing() {
    covers!("rules::production::strategy_cards::Research");
    let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheXxchaKingdom, Faction::TheArborec])
        .control(Planet::Starpoint, 0)
        .map_from(&[system_of(Planet::Starpoint)])
        .unit(system_of(Planet::Starpoint), Some(Planet::Starpoint), Unit::InfantryI, 0, 2)
        .build();
    set_leader(&mut s, 0, Leader::Ssruu, LeaderStatus::Readied);
    let mut rig = Rig::from_parts(s, k, Research::new(Seat(0), false, 4, ti4_engine_core_tests::inv::ALL_SOURCES));
    rig.run();
    assert!(rig.is_done(), "no Jol-Nar agent at the table: nothing to copy, and four resources are not at hand");
}
