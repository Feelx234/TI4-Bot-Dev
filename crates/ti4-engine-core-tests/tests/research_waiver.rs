//! Lane G5: the research window asks the faction waiver and its payment (`strategy_cards::resolve_research`): a technology only a waiver opens,
//! the waiver question, the payment question, the declines (the technology is not offered again; the paid research gives the bill back), a
//! payment the module refuses, and two modules' waivers on one technology.

use ti4_engine_core::choice::Subtype;
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub, Waiver};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Unit};
use ti4_engine_core::hooks::ResearchWaiver;
use ti4_engine_core::kernel::{Cx, Kernel};
use ti4_engine_core::rules::production::strategy_cards::Research;
use ti4_engine_core::state::CoreState;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

const WALLED: Technology = Technology::GravityDrive;
const OPEN: Technology = Technology::AntimassDeflectors;

fn w(code: u16) -> Waiver {
    Waiver { module: Faction::TheVuilraithCabal, code }
}

fn offers(_: &CoreState, _: &Kernel, _: Seat, t: Technology) -> Option<ResearchWaiver> {
    (t == WALLED).then(|| ResearchWaiver { waiver: w(0), payments: [w(1), w(2)].into_iter().collect() })
}

/// A second module offering a waiver on the same technology.
fn offers_too(_: &CoreState, _: &Kernel, _: Seat, t: Technology) -> Option<ResearchWaiver> {
    (t == WALLED).then(|| ResearchWaiver { waiver: Waiver { module: Faction::TheYinBrotherhood, code: 0 }, payments: SmallList::new() })
}

/// Payment `1` is paid (a captured cruiser changes hands), anything else is refused.
fn paid(cx: &mut Cx<'_>, who: Seat, _: Technology, payment: Waiver) -> bool {
    if payment != w(1) {
        return false;
    }
    cx.state.players_mut().by_seat_mut(who).unwrap().misc.captured_units.push((Seat(1), Unit::CruiserI));
    true
}

fn scenario() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7)
}

/// A module's offer of a waiver for a technology.
type Offer = fn(&CoreState, &Kernel, Seat, Technology) -> Option<ResearchWaiver>;

fn kernel(extra: &[Offer]) -> impl FnOnce(Kernel) -> Kernel + '_ {
    move |mut k| {
        k.hooks.strategy.research_waiver_offer.extend_from_slice(extra);
        k.hooks.strategy.research_waiver_paid.push(paid);
        k
    }
}

fn rig(window: Research, extra: &[Offer], goods: i16) -> Rig<Research> {
    let (s, k) = scenario().goods(0, goods).build();
    Rig::from_parts(s, kernel(extra)(k), window)
}

fn rt(t: Technology) -> Opt {
    Opt::Strategy(SOpt::Research(t))
}

fn owned(rig: &Rig<Research>, t: Technology) -> bool {
    rig.state.players().by_seat(Seat(0)).unwrap().tech.owned.contains(t)
}

fn captured(rig: &Rig<Research>) -> usize {
    rig.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.len()
}

fn primary() -> Research {
    Research::new(Seat(0), true, 6, ALL_SOURCES)
}

fn secondary() -> Research {
    Research::new(Seat(0), false, 4, ALL_SOURCES)
}

#[test]
fn a_technology_only_a_waiver_opens_asks_the_waiver_then_the_payment_and_researches_it() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = rig(primary(), &[offers], 0);
    rig.run();
    assert!(rig.opts().contains(&rt(WALLED)), "the waiver opens the technology");
    rig.pick_opt(rt(WALLED));
    let asked = rig.asked().clone();
    assert_eq!(asked.ask, Ask::Strategy(SAsk::ResearchWaiver { tech: WALLED, offered: [Some(w(0)), None] }));
    assert_eq!(asked.ctx.subtype, Subtype::Strategy(SSub::ResearchWaiver));
    assert_eq!(rig.opts(), vec![Opt::Strategy(SOpt::ResearchWaiver(0)), Opt::Decline]);
    assert!(!owned(&rig, WALLED) && !rig.driver.window.failed, "nothing yet, and the window did not leave the path");
    rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(0)));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiverPayment { tech: WALLED, waiver: w(0) }));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Strategy(SSub::ResearchWaiverPayment));
    assert_eq!(rig.opts(), vec![Opt::Strategy(SOpt::ResearchPayment(w(1))), Opt::Strategy(SOpt::ResearchPayment(w(2))), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Strategy(SOpt::ResearchPayment(w(1))));
    assert!(owned(&rig, WALLED), "the technology is gained");
    assert_eq!(captured(&rig), 1, "the module's payment was made");
    assert_eq!(rig.driver.window.researched.as_slice(), &[WALLED]);
    assert!(!rig.driver.window.failed);
    assert!(rig.is_done(), "no resources: the paid research asks nothing");
}

#[test]
fn a_declined_waiver_or_payment_of_the_free_research_leaves_the_technology_out_of_the_next_offer() {
    covers!("rules::production::strategy_cards::Research");
    for at_payment in [false, true] {
        let mut rig = rig(primary(), &[offers], 0);
        rig.run();
        rig.pick_opt(rt(WALLED));
        if at_payment {
            rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(0)));
        }
        rig.decline();
        let again = rig.asked().clone();
        assert!(matches!(again.ask, Ask::Strategy(SAsk::ResearchTechnology { cost: 0, decline: false, .. })), "{:?}", again.ask);
        assert!(!rig.opts().contains(&rt(WALLED)), "the declined technology is not offered again");
        assert!(rig.opts().contains(&rt(OPEN)));
        assert!(!owned(&rig, WALLED) && captured(&rig) == 0, "nothing was researched or paid");
        rig.pick_opt(rt(OPEN));
        assert!(owned(&rig, OPEN));
        assert!(rig.is_done());
    }
}

#[test]
fn the_paid_research_pays_first_and_a_declined_waiver_gives_the_position_back() {
    covers!("rules::production::strategy_cards::Research");
    for at_payment in [false, true] {
        let mut rig = rig(secondary(), &[offers], 4);
        rig.run();
        let before = rig.state.clone();
        rig.pick_opt(rt(WALLED));
        // The bill is paid (the four trade goods) before the waiver is asked.
        while !matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiver { .. })) {
            rig.pick(0);
        }
        assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "the bill is paid when the waiver is asked");
        if at_payment {
            rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(0)));
        }
        rig.decline();
        assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { decline: true, .. })), "{:?}", rig.asked().ask);
        assert!(!rig.opts().contains(&rt(WALLED)), "not offered again");
        assert_eq!(rig.state, before, "the position from before the bill is back (at_payment {at_payment})");
        rig.decline();
        assert!(rig.is_done());
        assert_eq!(rig.state, before);
    }
    // Accepted: paid and researched.
    let mut rig = rig(secondary(), &[offers], 4);
    rig.run();
    rig.pick_opt(rt(WALLED));
    while !matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiver { .. })) {
        rig.pick(0);
    }
    rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(0)));
    rig.pick_opt(Opt::Strategy(SOpt::ResearchPayment(w(1))));
    assert!(owned(&rig, WALLED));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "the bill stays paid");
    assert!(rig.is_done());
}

#[test]
fn a_payment_the_module_refuses_researches_nothing_and_changes_nothing() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = rig(primary(), &[offers], 0);
    rig.run();
    let before = rig.state.clone();
    rig.pick_opt(rt(WALLED));
    rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(0)));
    rig.pick_opt(Opt::Strategy(SOpt::ResearchPayment(w(2))));
    assert!(!owned(&rig, WALLED));
    assert_eq!(rig.state, before);
    assert!(!rig.opts().contains(&rt(WALLED)), "the technology is declined like a refused waiver");
}

#[test]
fn every_module_offers_its_waiver_in_the_order_of_the_modules() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = rig(primary(), &[offers, offers_too], 0);
    rig.run();
    rig.pick_opt(rt(WALLED));
    let yin = Waiver { module: Faction::TheYinBrotherhood, code: 0 };
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiver { tech: WALLED, offered: [Some(w(0)), Some(yin)] }));
    assert_eq!(rig.opts(), vec![Opt::Strategy(SOpt::ResearchWaiver(0)), Opt::Strategy(SOpt::ResearchWaiver(1)), Opt::Decline]);
    rig.pick_opt(Opt::Strategy(SOpt::ResearchWaiver(1)));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::ResearchWaiverPayment { tech: WALLED, waiver: yin }));
    assert_eq!(rig.opts(), vec![Opt::Decline], "the second module has no payment to name");
}

#[test]
fn more_waivers_than_the_question_can_name_leave_the_path_the_window_carries() {
    covers!("rules::production::strategy_cards::Research");
    let mut rig = rig(primary(), &[offers, offers_too, offers], 0);
    rig.run();
    rig.pick_opt(rt(WALLED));
    assert!(rig.driver.window.failed, "three modules: the run is the string's");
    assert!(rig.is_done());
}
