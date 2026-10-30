//! D4-2a: `rules::production::strategy_cards::Leadership` (three tokens for the primary, then tokens bought at three influence each).

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::rules::production::strategy_cards::{INFLUENCE_PER_TOKEN, LEADERSHIP_TOKENS, Leadership};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{planets_with_influence, systems_of};

fn strat(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

fn table(influence: &[u8], goods: i16) -> Scenario {
    let planets = planets_with_influence(influence);
    let mut sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&systems_of(&planets)).goods(0, goods).tokens(0, 1, 1, 1);
    for p in &planets {
        sc = sc.control(*p, 0);
    }
    sc
}

fn sheet(s: &CoreState) -> i16 {
    let e = s.players().by_seat(Seat(0)).unwrap().econ;
    e.tactic_tokens + e.fleet_tokens + e.strategic_tokens
}

#[test]
fn the_primary_gives_three_tokens_one_pool_at_a_time_and_then_offers_to_buy_more() {
    covers!("rules::production::strategy_cards::Leadership");
    assert_eq!((INFLUENCE_PER_TOKEN, LEADERSHIP_TOKENS), (3, 3));
    let mut rig = Rig::new(table(&[2, 3], 0), Leadership::new(Seat(0), true));
    rig.run();
    for left in [3u8, 2, 1] {
        assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left }));
        assert_eq!(rig.opts(), vec![strat(SOpt::Pool(Pool::Tactic)), strat(SOpt::Pool(Pool::Fleet)), strat(SOpt::Pool(Pool::Strategic))]);
        rig.pick(left as usize % 3);
    }
    assert_eq!(sheet(&rig.state), 3 + 3, "three tokens joined the three on the sheet");
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::LeadershipBuy { owed: 3, more: false }));
    assert_eq!(rig.opts(), vec![strat(SOpt::Buy(false)), strat(SOpt::Buy(true))]);
    rig.expect_illegal(Pick::Opt(strat(SOpt::Pool(Pool::Tactic))));
    rig.pick_opt(strat(SOpt::Buy(false)));
    assert!(rig.is_done());
}

#[test]
fn without_three_influence_to_spare_the_offer_is_never_made() {
    covers!("rules::production::strategy_cards::Leadership");
    let mut rig = Rig::new(table(&[2], 0), Leadership::new(Seat(0), true));
    rig.run();
    rig.pick(0).pick(0).pick(0);
    assert!(rig.is_done(), "two influence cannot buy a token");
    assert_eq!(sheet(&rig.state), 6);
}

#[test]
fn buying_a_token_spends_three_influence_in_planets_or_goods_and_gives_one_token() {
    covers!("rules::production::strategy_cards::Leadership");
    let planets = planets_with_influence(&[2, 3]);
    let mut rig = Rig::new(table(&[2, 3], 1), Leadership::new(Seat(0), true));
    rig.run();
    rig.pick(0).pick(0).pick(0);
    rig.pick_opt(strat(SOpt::Buy(true)));
    rig.assert_options_sound();
    rig.auto(ti4_engine_core_tests::rig::Policy::First);
    assert!(rig.is_done());
    assert_eq!(sheet(&rig.state), 3 + 3 + 1, "one bought token");
    let exhausted = planets.iter().filter(|p| rig.state.board().planet(**p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)).count();
    let goods_spent = 1 - rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods;
    assert!(exhausted > 0 || goods_spent > 0, "something paid for it");
}

#[test]
fn the_secondary_buys_one_token_for_three_influence_and_asks_nothing_when_it_cannot() {
    covers!("rules::production::strategy_cards::Leadership");
    let mut rig = Rig::new(table(&[3], 0), Leadership::new(Seat(0), false));
    rig.run();
    rig.auto(ti4_engine_core_tests::rig::Policy::First);
    assert!(rig.is_done());
    assert_eq!(sheet(&rig.state), 3 + 1);
    let mut poor = Rig::new(table(&[2], 0), Leadership::new(Seat(0), false));
    poor.run();
    assert!(poor.is_done());
    assert_eq!(delta(poor.start_state(), &poor.state), "", "two influence: nothing happens");
}

#[test]
fn leadership_smoke() {
    smoke("leadership/primary", || Rig::new(table(&[2, 3], 2), Leadership::new(Seat(0), true)));
    smoke("leadership/secondary", || Rig::new(table(&[1, 2], 1), Leadership::new(Seat(0), false)));
}
